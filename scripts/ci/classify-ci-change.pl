#!/usr/bin/env perl
# Shared companions: scripts/ci/shared-package-version.pl scripts/ci/StableVersion.pm
use strict;
use warnings;
use Getopt::Long qw(GetOptions);
use FindBin;
use lib $FindBin::Bin;
use StableVersion qw(stable version_cmp);

# Conservative, offline selection. A failed observation always retains full CI.
my ($base, $head, $event, $ref, $policy, $allow_version_bump);
GetOptions('base=s' => \$base, 'head=s' => \$head, 'event=s' => \$event,
    'ref=s' => \$ref, 'policy=s' => \$policy,
    'allow-version-bump' => \$allow_version_bump) && !@ARGV or die "invalid arguments\n";
sub git_read {
    open my $fh, '-|', 'git', @_ or die "cannot start git\n";
    local $/;
    my $value = <$fh>;
    close $fh or die "git observation failed\n";
    return $value // '';
}
sub path_valid {
    my ($path) = @_;
    return $path ne '' && $path !~ m{(?:\A/|[\x00-\x1f\x7f]|(?:\A|/)\.\.?(/|\z)|//|/\z)};
}
sub version_pair {
    my ($revision) = @_;
    my $manifest=git_read('ls-tree','--name-only',$revision,'--','Cargo.toml');
    my $version_path=length($manifest) ? 'Cargo.toml' : 'VERSION';
    for my $path ($version_path, 'CHANGELOG.md') {
        my $entry = git_read('ls-tree', '-z', $revision, '--', $path);
        die "version pair must be regular non-executable files\n"
            unless $entry =~ /\A100644 blob [0-9a-f]+\t\Q$path\E\0\z/;
    }
    my $input=git_read('show',"$revision:$version_path");
    my $version=$input;
    my $normalized='';
    if ($version_path eq 'Cargo.toml') {
        require IPC::Open2;
        my $pid=IPC::Open2::open2(my $out,my $in,$^X,"$FindBin::Bin/shared-package-version.pl",'-');
        print {$in} $input; close $in or die "manifest reader input failed\n";
        { local $/; $version=<$out>; } close $out; waitpid($pid,0);
        $?==0 or die "invalid package manifest\n";
        $version =~ s/\n\z//;
        $normalized=$input;
        $normalized =~ s/^(version[ \t]*=[ \t]*")\Q$version\E(")/${1}0.0.0$2/m;
    } else { $version =~ s/\n\z//; }
    die "invalid version\n" unless stable($version);
    my $notes = git_read('show', "$revision:CHANGELOG.md");
    my @headings = $notes =~ /^(##[ \t]+[^\n]*)/mg;
    my ($top, $suffix) = ($headings[0] // '') =~ /\A##[ \t]+\[([^]]+)\](?:[ \t]+-[ \t]+(Unreleased|[0-9]{4}-[0-9]{2}-[0-9]{2}))?[ \t]*\z/;
    die "VERSION and changelog disagree\n" unless defined($top) && $top eq $version;
    die "pending version needs previous release\n"
        if defined($suffix) && $suffix eq 'Unreleased' && @headings < 2;
    for my $heading (@headings[1 .. $#headings]) {
        my ($older) = $heading =~ /\A##[ \t]+\[([^]]+)\]/;
        die "invalid or misplaced historical version\n"
            unless stable($older) && version_cmp($older, $version) < 0;
    }
    return ($version,$version_path,$normalized);
}
sub classify {
    die "unsupported event or ref\n" unless defined $event && defined $ref
        && ($event eq 'pull_request' || $event eq 'push') && $ref =~ m{\Arefs/heads/[^\x00-\x20]+\z};
    die "missing commit identities\n" unless defined $base && defined $head
        && $base =~ /\A[0-9a-f]{40}(?:[0-9a-f]{24})?\z/
        && $head =~ /\A[0-9a-f]{40}(?:[0-9a-f]{24})?\z/;
    die "invalid policy path\n" unless defined $policy && path_valid($policy);
    git_read('cat-file', '-e', "$base^{commit}");
    git_read('cat-file', '-e', "$head^{commit}");
    my $start = $base;
    if ($event eq 'pull_request') {
        $start = git_read('merge-base', $base, $head);
        chomp $start;
        die "ambiguous merge base\n" unless $start =~ /\A[0-9a-f]{40}(?:[0-9a-f]{24})?\z/;
    } else {
        git_read('merge-base', '--is-ancestor', $base, $head);
    }
    for my $revision ($start, $head) {
        my $entry = git_read('ls-tree', '-z', $revision, '--', $policy);
        die "policy must be a regular non-executable file\n"
            unless $entry =~ /\A100644 blob [0-9a-f]+\t\Q$policy\E\0\z/;
    }
    my $rules = git_read('show', "$start:$policy");
    die "policy changed\n" unless $rules eq git_read('show', "$head:$policy");
    my %allowed;
    for my $path (split /\n/, $rules) {
        next if $path eq '' || $path =~ /^#/;
        die "invalid policy entry\n" unless path_valid($path) && $path =~ /\.md\z/
            && $path !~ m{\A(?:\.github|scripts|ci|make)/} && $path ne $policy;
        die "duplicate policy entry\n" if $allowed{$path}++;
    }
    die "empty policy\n" unless %allowed;
    my @records = split /\0/, git_read('diff', '--raw', '-z', '--no-renames', '--no-abbrev',
        '--ignore-submodules=none', '--no-relative', $start, $head, '--');
    die "empty comparison\n" unless @records;
    my %changed;
    while (@records) {
        my $metadata = shift @records;
        my $path = shift @records;
        die "unknown diff record\n" unless defined $path && path_valid($path)
            && ($allowed{$path} || ($allow_version_bump && ($path eq 'VERSION' || $path eq 'Cargo.toml')))
            && $metadata =~ /\A:(000000|100644) (000000|100644) [0-9a-f]+ [0-9a-f]+ [AMD]\z/;
        $changed{$path} = 1;
    }
    if ($changed{VERSION} || $changed{'Cargo.toml'}) {
        die "version bump requires reviewed changelog changes\n"
            unless $allowed{'CHANGELOG.md'} && $changed{'CHANGELOG.md'};
        my ($before,$before_path,$before_manifest)=version_pair($start);
        my ($after,$after_path,$after_manifest)=version_pair($head);
        die "version source or other package metadata changed\n" unless $before_path eq $after_path && $before_manifest eq $after_manifest;
        die "VERSION must advance\n" unless version_cmp($after, $before) > 0;
    }
    return 'docs';
}
my $scope = eval { classify() };
if (!defined $scope) { warn "Full CI selected: $@"; $scope = 'full'; }
print "$scope\n";
