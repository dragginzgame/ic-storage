#!/usr/bin/env perl
# Offline diagnostic only. Function ownership includes inlined code; it is not
# a causal component-cost estimate. GNU c++filt with Rust support is explicit.
use strict;
use warnings;
use JSON::PP;
use IPC::Open2;

@ARGV == 1 or die "usage: inspect-wasm-size.pl MODULE.wasm\nRequires c++filt -s rust on PATH (or CXXFILT_BIN).\n";
open my $file, '<:raw', $ARGV[0] or die "$ARGV[0]: $!\n";
my $bytes = do { local $/; <$file> };
close $file or die "$!\n";
substr($bytes, 0, 8) eq "\0asm\1\0\0\0" or die "unsupported Wasm header\n";
sub uleb {
    my ($pos) = @_;
    my ($value, $shift) = (0, 0);
    while (1) {
        $$pos < length($bytes) && $shift < 64 or die "invalid LEB\n";
        my $byte = ord substr($bytes, $$pos, 1); ++$$pos;
        $value |= ($byte & 127) << $shift;
        return $value if $byte < 128;
        $shift += 7;
    }
}
sub name {
    my ($pos) = @_;
    my $length = uleb($pos);
    $$pos + $length <= length($bytes) or die "name outside module\n";
    my $name = substr($bytes, $$pos, $length); $$pos += $length;
    return $name;
}
my (@sections, @functions, %names);
my ($pos, $imports) = (8, 0);
while ($pos < length($bytes)) {
    my $start = $pos;
    my $id = ord substr($bytes, $pos++, 1);
    my $length = uleb(\$pos);
    my $end = $pos + $length;
    $end <= length($bytes) or die "section outside module\n";
    my $label = "$id";
    if ($id == 0) {
        $label = name(\$pos);
        if ($label eq 'name') {
            while ($pos < $end) {
                my $kind = ord substr($bytes, $pos++, 1);
                my $size = uleb(\$pos); my $stop = $pos + $size;
                $stop <= $end or die "name subsection outside section\n";
                if ($kind == 1) {
                    my $count = uleb(\$pos);
                    for (1..$count) { my $index = uleb(\$pos); $names{$index} = name(\$pos); }
                }
                $pos = $stop;
            }
        }
    } elsif ($id == 2) {
        my $count = uleb(\$pos);
        for (1..$count) {
            name(\$pos); name(\$pos);
            my $kind = ord substr($bytes, $pos++, 1);
            $kind == 0 or die "diagnostic currently supports function-only imports\n";
            uleb(\$pos); ++$imports;
        }
    } elsif ($id == 10) {
        my $count = uleb(\$pos);
        for my $index ($imports..$imports+$count-1) {
            my $before = $pos; my $size = uleb(\$pos);
            $pos+$size <= $end or die "function outside code section\n";
            push @functions, {index=>$index, body_bytes=>$size, encoded_bytes=>$pos+$size-$before};
            $pos += $size;
        }
        $pos == $end or die "code section remainder\n";
    }
    push @sections, {id=>$id, name=>$label, payload_bytes=>$length, encoded_bytes=>$end-$start};
    $pos = $end;
}
# Feed symbols concurrently so long generic names cannot deadlock two pipes.
# Inspection writes no temporary files and never changes the module.
pipe(my $input, my $writer) or die "$!\n";
my $writer_pid = fork(); defined $writer_pid or die "$!\n";
if (!$writer_pid) {
    close $input;
    for my $function (@functions) {
        my $symbol = $names{$function->{index}} // "function_$function->{index}";
        $symbol !~ /[\r\n]/ or die "invalid symbol newline\n";
        print {$writer} "$symbol\n" or die "$!\n";
    }
    close $writer or die "$!\n";
    exit 0;
}
close $writer;
my $demangler = $ENV{CXXFILT_BIN} // 'c++filt';
my $pid = open2(my $output, '<&'.fileno($input), $demangler, '-s', 'rust');
my %groups;
for my $function (@functions) {
    my $name = <$output>; defined $name or die "missing demangler output\n";
    chomp $name; $name =~ s/\[[0-9a-f]+\]//g;
    $name !~ /^_R/ or die "demangler does not support this Rust symbol\n";
    $function->{name} = $name;
    my $group = 'other';
    if ($name =~ /^<?(?:storage_size_consumer|icss_size_consumer)::/) { $group='consumer'; }
    elsif ($name =~ /^<?ic_storage::engine/) { $group='engine'; }
    elsif ($name =~ /^<?ic_storage::adapter/) { $group='adapters'; }
    elsif ($name =~ /^<?ic_storage::/) { $group='Storage runtime / codecs'; }
    elsif ($name =~ /^<?ic_stable_structures::memory_manager/) { $group='ICSS memory manager'; }
    elsif ($name =~ /^<?ic_stable_structures::/) { $group='engine'; }
    elsif ($name =~ /^<?(?:ic_wire|serde|serde_core|postcard|cobs)::/) { $group='serialization'; }
    elsif ($name =~ /^<?(?:core|alloc|std)::/) {
        $group='Rust helpers';
        $group='Rust sorting' if $name =~ /::sort::/;
        $group='Rust heap B-trees' if $name =~ /alloc::collections::btree/;
    }
    $function->{owner_group} = $group;
    $groups{$group} += $function->{encoded_bytes};
}
close $output; waitpid $pid, 0; $? == 0 or die "demangler failed\n";
close $input; waitpid $writer_pid, 0; $? == 0 or die "symbol input failed\n";
my $sum=8; $sum += $_->{encoded_bytes} for @sections;
$sum == length($bytes) or die "section accounting mismatch\n";
print JSON::PP->new->canonical->pretty->encode({
    module=>$ARGV[0], module_bytes=>length($bytes), sections=>\@sections,
    named=>scalar(keys %names)>0 ? JSON::PP::true : JSON::PP::false,
    function_owner_bytes=>\%groups, functions=>\@functions,
    caveat=>'Symbol owners include inlined work; diagnostic builds can differ from stripped builds. Compare section sizes before attribution.',
});
