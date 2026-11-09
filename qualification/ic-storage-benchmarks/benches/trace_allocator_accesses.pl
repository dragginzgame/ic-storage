#!/usr/bin/env perl
# Optional second diagnostic pass over trace_stable_reads.pl output (Binaryen 133).
# Observe expensive allocator loads/stores without adding linear-memory storage.
use strict;
use warnings;
@ARGV == 2 or die "usage: $0 traced.wat output.wat\n";
open my $input, '<', $ARGV[0] or die "$ARGV[0]: $!";
my $wat = do { local $/; <$input> };
close $input;
$wat =~ /\(global \$read_trace_active / or die "missing first diagnostic pass\n";
$wat !~ /\$allocator_trace_/ or die "already instrumented\n";
sub imported {
    my ($name) = @_;
    my @names = $wat =~ /^ \(import "ic0" "\Q$name\E" \(func (\$[^\s()]+)/mg;
    @names == 1 or die "expected one ic0.$name import\n";
    return $names[0];
}
my ($counter, $append, $reply) = map { imported($_) }
    qw(performance_counter msg_reply_data_append msg_reply);
my @sites;
my $extra = '';
my %kinds = ( 'i32.load'=>0, 'i32.store'=>1, 'i32.load8_u'=>2,
    'i32.store8'=>3, 'i64.load'=>4, 'i64.store'=>5 );
sub rewrite {
    my ($node, $owner) = @_;
    my ($op, @args) = @$node;
    my @values = map { ref($_) ? rewrite($_, $owner) : $_ } @args;
    if (exists $kinds{$op}) {
        my @attributes;
        push @attributes, shift @values while @values && $values[0] =~ /^(?:offset|align)=/;
        my ($offset) = map { /^offset=(\d+)$/ ? $1 : () } @attributes;
        $offset //= 0;
        my $store = $op =~ /store/;
        @values == ($store ? 2 : 1) or die "unexpected $op operands\n";
        my $type = $op =~ /^i64/ ? 'i64' : 'i32';
        my $id = @sites;
        push @sites, [$id, $owner, $op, $offset, join(' ', @attributes)];
        my $name = "\$allocator_trace_site_$id";
        my $signature = $store ? "(param \$value $type)" : "(result $type)";
        my $operation = "($op @attributes (local.get \$address)";
        $operation .= ' (local.get $value)' if $store;
        $operation .= ')';
        $operation = "(local.set \$value $operation)" unless $store;
        $extra .= " (func $name (param \$address i32) $signature\n";
        $extra .= " (local \$value $type)\n" unless $store;
        $extra .= <<"WAT";
 (local \$start i64) (local \$cost i64)
 (local.set \$start (call $counter (i32.const 0)))
 $operation
 (local.set \$cost (i64.sub (call $counter (i32.const 0)) (local.get \$start)))
 (if (i64.ge_u (local.get \$cost) (i64.const 10000)) (then
  (call \$allocator_trace_save
    (i64.add (i64.extend_i32_u (local.get \$address)) (i64.const $offset))
    (i64.add (i64.const $kinds{$op}) (i64.mul (global.get \$read_trace_counters) (i64.const 16)))
    (local.get \$cost) (i64.const $id))))
WAT
        $extra .= '(local.get $value)' unless $store;
        $extra .= ")\n";
        return "(call $name @values)";
    }
    $op !~ /^(?:i32|i64|f32|f64)\.(?:load|store)|^memory\.(?:copy|fill)/
        or die "unhandled allocator memory instruction: $op\n";
    return "($op @values)";
}
sub parse {
    my ($tokens, $position) = @_;
    $tokens->[$$position++] eq '(' or die "expected opening parenthesis\n";
    my @node;
    while ($$position < @$tokens && $tokens->[$$position] ne ')') {
        push @node, $tokens->[$$position] eq '(' ? parse($tokens, $position) : $tokens->[$$position++];
    }
    $$position < @$tokens or die "unclosed expression\n";
    $$position++;
    return \@node;
}
my $functions = 0;
$wat =~ s{(^ \(func (\$\S*8dlmalloc\S*)[^\n]*\n.*?)(?=^ \(func |\z)}{
    my ($body, $owner) = ($1, $2);
    $body !~ /[";]/ or die "unexpected string/comment in allocator function\n";
    my @tokens = $body =~ /(\(|\)|[^\s()]+)/g;
    my $position = 0;
    my $tree = parse(\@tokens, \$position);
    $position == @tokens or die "unexpected content after allocator function\n";
    $functions++;
    ' ' . rewrite($tree, $owner) . "\n";
}mgse;
$functions == 6 or die "expected six dlmalloc functions, got $functions\n";
$extra .= ' (global $allocator_trace_count (mut i64) (i64.const 0))' . "\n";
for my $i (0..63) {
    for my $field (qw(address kind cost site)) {
        $extra .= " (global \$allocator_trace_${field}_$i (mut i64) (i64.const 0))\n";
    }
}
sub save_tree {
    my ($lo, $hi) = @_;
    return join '', map { "(global.set \$allocator_trace_${_}_$lo (local.get \$$_))\n" }
        qw(address kind cost site) if $hi - $lo == 1;
    my $mid = int(($lo + $hi) / 2);
    return "(if (i64.lt_u (global.get \$allocator_trace_count) (i64.const $mid)) (then\n"
        . save_tree($lo, $mid) . ") (else\n" . save_tree($mid, $hi) . "))\n";
}
$extra .= ' (func $allocator_trace_save (param $address i64) (param $kind i64) (param $cost i64) (param $site i64)' . "\n"
    . '(if (i64.lt_u (global.get $allocator_trace_count) (i64.const 64)) (then' . "\n"
    . save_tree(0, 64) . "))\n"
    . '(global.set $allocator_trace_count (i64.add (global.get $allocator_trace_count) (i64.const 1))))' . "\n";
$wat =~ s/(^ \(func \$read_trace_run\s*)/$1(global.set \$allocator_trace_count (i64.const 0))\n/m
    or die "missing benchmark reset\n";
$extra .= <<'WAT';
 (func $allocator_trace_query (local $scratch i32)
  (local.set $scratch (i32.sub (global.get $__stack_pointer) (i32.const 32)))
  (i64.store (local.get $scratch) (global.get $allocator_trace_count))
WAT
$extra .= " (call $append (local.get \$scratch) (i32.const 8))\n";
for my $i (0..63) {
    my $offset = 0;
    for my $field (qw(address kind cost site)) {
        $extra .= " (i64.store offset=$offset (local.get \$scratch) (global.get \$allocator_trace_${field}_$i))\n";
        $offset += 8;
    }
    $extra .= " (call $append (local.get \$scratch) (i32.const 32))\n";
}
$extra .= " (call $reply))\n" . ' (export "canister_query allocator_access_trace" (func $allocator_trace_query))' . "\n";
$wat =~ s/\)\s*\z/$extra)\n/ or die "missing module end\n";
open my $output, '>', $ARGV[1] or die "$ARGV[1]: $!";
print {$output} $wat;
close $output or die "$ARGV[1]: $!";
open my $map, '>', "$ARGV[1].sites.tsv" or die "$ARGV[1].sites.tsv: $!";
print {$map} join("\t", @$_), "\n" for @sites;
close $map or die "$ARGV[1].sites.tsv: $!";
printf "instrumented %d allocator sites in %d functions\n", scalar(@sites), $functions;
