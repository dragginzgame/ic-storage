#!/usr/bin/env perl
# Diagnostic only. Input/output are Binaryen 133 folded WAT, not production Wasm.
# Append globals/functions without changing existing memory, data or globals.
use strict;
use warnings;
my $mode = @ARGV == 3 ? pop(@ARGV) : '';
$mode eq '' || $mode eq '--execution-only' || $mode eq '--batch-reads'
    or die "unknown mode: $mode\n";
my $execution_only = $mode eq '--execution-only';
my $batch_reads = $mode eq '--batch-reads';
my $capacity = 160;
@ARGV == 2 or die "usage: $0 input.wat output.wat [--execution-only|--batch-reads]\n";
open my $input, '<', $ARGV[0] or die "$ARGV[0]: $!";
my $wat = do { local $/; <$input> };
close $input;
$wat !~ /\$read_trace_/ or die "already instrumented\n";
sub imported {
    my ($name) = @_;
    my @names = $wat =~ /^ \(import "ic0" "\Q$name\E" \(func (\$[^\s()]+)/mg;
    @names == 1 or die "expected exactly one ic0.$name import\n";
    return $names[0];
}
my ($read, $counter, $append, $reply) = map { imported($_) }
    qw(stable64_read performance_counter msg_reply_data_append msg_reply);
if ($execution_only) {
    $wat =~ /^ \(global \$__stack_pointer \(mut i32\) \(i32.const 8388608\)\)$/m
        or die "expected benchmark's existing 8 MiB stack\n";
    $wat =~ s/\(export "canister_update benchmark" \(func \$"canister_update benchmark"\)\)/'(export "canister_update benchmark" (func $read_trace_execution))'/e
        or die "missing benchmark export\n";
    my $extra = <<"WAT";
 (global \$read_trace_execution_cost (mut i64) (i64.const 0))
 (func \$read_trace_execution (local \$start i64)
  (local.set \$start (call $counter (i32.const 0)))
  (call \$"canister_update benchmark")
  (global.set \$read_trace_execution_cost (i64.sub (call $counter (i32.const 0)) (local.get \$start))))
 (func \$read_trace_execution_query (local \$scratch i32)
  (local.set \$scratch (i32.sub (global.get \$__stack_pointer) (i32.const 8)))
  (i64.store (local.get \$scratch) (global.get \$read_trace_execution_cost))
  (call $append (local.get \$scratch) (i32.const 8))
  (call $reply))
 (export "canister_query benchmark_execution_cost" (func \$read_trace_execution_query))
WAT
    $wat =~ s/\)\s*\z/$extra)\n/ or die "missing module end\n";
    open my $output, '>', $ARGV[1] or die "$ARGV[1]: $!";
    print {$output} $wat;
    close $output or die "$ARGV[1]: $!";
    print "instrumented execution boundary only; no added linear memory\n";
    exit 0;
}
my %heap;
for my $kind (qw(malloc free)) {
    my @names = $wat =~ /^ \(func (\$\S*8dlmalloc\S*\d${kind}\S*) \(param \$0 i32\)/mg;
    @names == 1 or die "expected one dlmalloc.$kind function\n";
    $heap{$kind} = $names[0];
    $wat =~ s/\(call \Q$names[0]\E(?=\s)/"(call \$read_trace_$kind"/ge
        or die "no $kind calls\n";
}
$wat =~ s/\(call \Q$counter\E(?=\s)/'(call $read_trace_counter'/ge
    or die "no measurement counters\n";
$wat =~ /^ \(global \$__stack_pointer \(mut i32\) \(i32.const 8388608\)\)$/m
    or die "expected benchmark's existing 8 MiB stack\n";
my $calls = $wat =~ s/\(call \Q$read\E(?=\s)/'(call $read_trace_read'/ge;
$calls > 0 or die "no direct stable reads\n";
$wat =~ s/\(export "canister_update benchmark" \(func \$"canister_update benchmark"\)\)/'(export "canister_update benchmark" (func $read_trace_run))'/e
    or die "missing benchmark export\n";
my $extra = ' (global $read_trace_count (mut i64) (i64.const 0))' . "\n";
for my $name (qw(active counters malloc_calls malloc_cost free_calls free_cost)) {
    $extra .= " (global \$read_trace_$name (mut i64) (i64.const 0))\n";
}
if ($batch_reads) {
    $extra .= " (global \$read_trace_total_calls (mut i64) (i64.const 0))\n"
        . " (global \$read_trace_total_bytes (mut i64) (i64.const 0))\n";
}
for my $i (0..$capacity - 1) {
    for my $field (qw(dst src len cost)) {
        $extra .= " (global \$read_trace_${field}_$i (mut i64) (i64.const 0))\n";
    }
}
# A balanced selection tree avoids a heap-side trace buffer and linear scans.
sub save_tree {
    my ($lo, $hi) = @_;
    if ($hi - $lo == 1) {
        return join '', map { "(global.set \$read_trace_${_}_$lo (local.get \$$_))\n" }
            qw(dst src len cost);
    }
    my $mid = int(($lo + $hi) / 2);
    return "(if (i64.lt_u (global.get \$read_trace_count) (i64.const $mid))\n(then\n"
        . save_tree($lo, $mid) . ") (else\n" . save_tree($mid, $hi) . "))\n";
}
$extra .= <<"WAT";
 (func \$read_trace_counter (param \$kind i32) (result i64)
  (global.set \$read_trace_counters (i64.add (global.get \$read_trace_counters) (i64.const 1)))
  (global.set \$read_trace_active (i64.xor (global.get \$read_trace_active) (i64.const 1)))
  (call $counter (local.get \$kind)))
WAT
for my $kind (qw(malloc free)) {
    my $result = $kind eq 'malloc' ? '(result i32)' : '';
    my $invoke = "(call $heap{$kind} (local.get \$arg))";
    $invoke = "(local.set \$result $invoke)" if $kind eq 'malloc';
    $extra .= <<"WAT";
 (func \$read_trace_$kind (param \$arg i32) $result
  (local \$start i64) (local \$cost i64) (local \$result i32)
  (local.set \$start (call $counter (i32.const 0)))
  $invoke
  (local.set \$cost (i64.sub (call $counter (i32.const 0)) (local.get \$start)))
  (if (i64.ne (global.get \$read_trace_active) (i64.const 0)) (then
   (global.set \$read_trace_${kind}_calls (i64.add (global.get \$read_trace_${kind}_calls) (i64.const 1)))
   (global.set \$read_trace_${kind}_cost (i64.add (global.get \$read_trace_${kind}_cost) (local.get \$cost)))))
WAT
    $extra .= '(local.get $result)' if $kind eq 'malloc';
    $extra .= ")\n";
}
my $record_condition = $batch_reads ? '(i64.ge_u (local.get $cost) (i64.const 10000))' : '(i32.const 1)';
my $totals = $batch_reads ? <<'WAT' : '';
  (global.set $read_trace_total_calls (i64.add (global.get $read_trace_total_calls) (i64.const 1)))
  (global.set $read_trace_total_bytes (i64.add (global.get $read_trace_total_bytes) (local.get $len)))
WAT
$extra .= <<"WAT";
 (func \$read_trace_read (param \$dst i64) (param \$src i64) (param \$len i64)
  (local \$start i64) (local \$cost i64)
  (local.set \$start (call $counter (i32.const 0)))
  (call $read (local.get \$dst) (local.get \$src) (local.get \$len))
  (local.set \$cost (i64.sub (call $counter (i32.const 0)) (local.get \$start)))
  $totals
  (if $record_condition (then
  (if (i64.lt_u (global.get \$read_trace_count) (i64.const $capacity)) (then
WAT
$extra .= save_tree(0, $capacity) . "))\n";
$extra .= <<'WAT';
  (global.set $read_trace_count (i64.add (global.get $read_trace_count) (i64.const 1))))))
 (func $read_trace_run
  (global.set $read_trace_count (i64.const 0))
  (global.set $read_trace_active (i64.const 0))
  (global.set $read_trace_counters (i64.const 0))
  (global.set $read_trace_malloc_calls (i64.const 0))
  (global.set $read_trace_malloc_cost (i64.const 0))
  (global.set $read_trace_free_calls (i64.const 0))
  (global.set $read_trace_free_cost (i64.const 0))
  (call $"canister_update benchmark"))
 (func $read_trace_query (local $scratch i32)
  (local.set $scratch (i32.sub (global.get $__stack_pointer) (i32.const 32)))
WAT
# Query-only writes below the idle stack pointer never commit to application heap.
$extra .= " (i64.store (local.get \$scratch) (global.get \$read_trace_count))\n"
    . " (call $append (local.get \$scratch) (i32.const 8))\n";
for my $i (0..$capacity - 1) {
    my $offset = 0;
    for my $field (qw(dst src len cost)) {
        $extra .= " (i64.store offset=$offset (local.get \$scratch) (global.get \$read_trace_${field}_$i))\n";
        $offset += 8;
    }
    $extra .= " (call $append (local.get \$scratch) (i32.const 32))\n";
}
for my $name (qw(counters malloc_calls malloc_cost free_calls free_cost)) {
    $extra .= " (i64.store (local.get \$scratch) (global.get \$read_trace_$name))\n"
        . " (call $append (local.get \$scratch) (i32.const 8))\n";
}
if ($batch_reads) {
    for my $name (qw(total_calls total_bytes)) {
        $extra .= " (i64.store (local.get \$scratch) (global.get \$read_trace_$name))\n"
            . " (call $append (local.get \$scratch) (i32.const 8))\n";
    }
    $extra =~ s/(\(func \$read_trace_run\n)/$1  (global.set \$read_trace_total_calls (i64.const 0))\n  (global.set \$read_trace_total_bytes (i64.const 0))\n/
        or die "missing read trace reset\n";
}
$extra .= " (call $reply))\n"
    . ' (export "canister_query stable_read_trace" (func $read_trace_query))' . "\n";
$wat =~ s/\)\s*\z/$extra)\n/ or die "missing module end\n";
open my $output, '>', $ARGV[1] or die "$ARGV[1]: $!";
print {$output} $wat;
close $output or die "$ARGV[1]: $!";
my $scope = $batch_reads ? 'reads costing >=10000 plus total calls/bytes' : 'all reads';
print "instrumented $calls stable-read call sites; $capacity records ($scope), no added linear memory\n";
