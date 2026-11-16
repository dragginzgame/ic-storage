#!/usr/bin/env perl
# Consumer-owned bounded setup/test diagnostics. Never copy raw evidence trees.
use strict;
use warnings;
use Cwd qw(abs_path);
use File::Find;
use File::Path qw(make_path);
use File::Spec;

sub regular_path {
    my ($path) = @_;
    my $absolute = File::Spec->rel2abs($path);
    my $part = '';
    for my $segment (split m{/}, $absolute) {
        next unless length $segment;
        $part .= "/$segment";
        return 0 if -l $part;
    }
    return -f $absolute;
}
sub sanitized {
    my ($path) = @_;
    die "refusing nonregular or symlinked evidence\n" unless regular_path($path);
    open my $in, '<:raw', $path or die "open selected diagnostic: $!\n";
    my $count = read($in, my $text, 1024 * 1024 + 1);
    defined $count or die "read selected diagnostic: $!\n";
    close $in or die "close diagnostic: $!\n";
    return "[binary diagnostic omitted]\n" if $text =~ /\x00/;
    return "[private-key diagnostic omitted in full]\n" if $text =~ /-----BEGIN .*PRIVATE KEY/;
    my $truncated = $count > 1024 * 1024;
    # Never expose a partial line/token at the input bound.
    $text =~ s/[^\n]*\z// if $truncated;
    $text =~ s/\e\[[0-9;]*[[:alpha:]]//g;
    $text =~ s/[\x01-\x08\x0b-\x1f\x7f]/?/g;
    for my $name (keys %ENV) {
        next unless $name =~ /(?:TOKEN|PASSWORD|PASSWD|SECRET|PRIVATE_KEY|API_KEY|ACCESS_KEY)/i;
        my $secret = $ENV{$name};
        next unless defined $secret && length($secret) >= 4;
        $text =~ s/\Q$secret\E/[REDACTED]/g;
    }
    # Drop whole sensitive-field lines, including shell/env and JSON spellings.
    $text =~ s/^.*(?:authorization|cookie|password|passwd|token|secret|private[_ -]?key|api[_ -]?key|access[_ -]?key)\s*["']?\s*[:=].*$/[sensitive field omitted]/gmi;
    $text =~ s{(https?://)[^/\s\@]+\@}{${1}[REDACTED]\@}g;
    $text =~ s{(https?://[^\s?]+)\?[^\s]+}{$1?[query omitted]}g;
    my @lines = split /(?<=\n)/, $text;
    my $out = '';
    for my $line (@lines) {
        if (length($out) + length($line) > 65408) { $truncated = 1; last; }
        $out .= $line;
    }
    $out .= "\n[diagnostic truncated: input 1 MiB, output 64 KiB limits]\n" if $truncated;
    return length($out) ? $out : "[diagnostic empty]\n";
}
sub write_file {
    my ($path, $text) = @_;
    die "refusing output symlink\n" if -l $path;
    open my $out, '>', $path or die "write diagnostic: $!\n";
    print {$out} $text or die "write diagnostic contents: $!\n";
    close $out or die "close diagnostic output: $!\n";
}
my $mode = shift @ARGV // '';
if ($mode eq 'excerpt') {
    @ARGV == 1 or die "excerpt INPUT\n";
    my $text = sanitized($ARGV[0]);
    # A bounded tail, already sanitized without cutting tokens at the input edge.
    my @lines = split /\n/, $text;
    print "[excerpt: last 30 sanitized lines]\n";
    print join("\n", @lines > 30 ? @lines[-30..-1] : @lines), "\n";
    exit 0;
}
$mode eq 'collect' && @ARGV == 4 or die "collect WORKSPACE RUNNER_TEMP TOOL_STORE OUTPUT\n";
my ($workspace, $temporary, $store, $output) = @ARGV;
die "output already exists\n" if -e $output || -l $output;
make_path($output);
my @selected;
my @missing;
my @roots = (
    ["$workspace/target/ci-setup", 'setup'],
    ["$workspace/target/toko-tooling", 'tooling'],
    ["$workspace/target/evidence", 'tests'],
    [$store, 'tool-attempts'],
);
for my $selection (@roots) {
    my ($root, $label) = @$selection;
    if (!-d $root || -l $root) { push @missing, $label; next; }
    find({ no_chdir => 1, follow => 0, wanted => sub {
        my $path = $File::Find::name;
        my $relative = $path eq $root ? '' : substr($path, length($root) + 1);
        if ($label eq 'tool-attempts' && -d $path && $path ne $root &&
            $relative !~ m{^rust-[A-Za-z0-9_-]+(?:/(?:set\.|\.attempt\.)[A-Za-z0-9]+)?$}) {
            $File::Find::prune = 1;
        }
        return if -l $path || !-f $path;
        # Explicit log/marker selection includes hidden attempt directories.
        return unless $relative =~ m{(?:^|/)(?:[A-Za-z0-9_.-]+\.(?:log|status)|COMPLETE|ADMITTED|RECOVERED|JOB_RECOVERED)$};
        return if $label eq 'tool-attempts' && $relative !~ m{^rust-[A-Za-z0-9_-]+/(?:set\.|\.attempt\.)[A-Za-z0-9]+/install\.log$};
        return unless regular_path($path);
        push @selected, ["$label/$relative", $path];
    }}, $root);
}
for my $name (qw(tools-install.log tools-check.log)) {
    my $path = "$temporary/$name";
    if (regular_path($path)) { push @selected, ["common-tools/$name", $path]; }
    else { push @missing, $name; }
}
my $index = "Sanitized diagnostics only; raw logs, binaries, payloads and environment dumps are omitted.\nLimits: 64 selected files, 64 KiB each, plus this index; approximately 4 MiB total.\n";
my $number = 0;
for my $selected (sort { $a->[0] cmp $b->[0] } @selected) {
    if ($number == 64) { $index .= "File selection truncated at 64 files.\n"; last; }
    my ($label, $path) = @$selected;
    my $name = sprintf('%03d.log', ++$number);
    write_file("$output/$name", sanitized($path));
    $label =~ s/[^A-Za-z0-9_.\/-]/?/g;
    $index .= "$name <- $label\n";
}
$index .= "Missing evidence (step may not have started): " . join(', ', @missing) . "\n" if @missing;
$index .= "No selected diagnostics were available.\n" if !$number;
write_file("$output/INDEX.txt", $index);
print $index;
exit($number ? 0 : 3);
