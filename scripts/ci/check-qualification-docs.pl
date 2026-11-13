#!/usr/bin/env perl
# Bound existing private protocol documentation debt by diagnostic identity.
use strict;
use warnings;
use JSON::PP;

@ARGV == 2 or die "usage: check-qualification-docs.pl BASELINE CARGO-JSON\n";
my ($baseline_path, $diagnostics_path) = @ARGV;
my $json = JSON::PP->new->canonical;
open my $baseline_file, '<', $baseline_path or die "$baseline_path: $!\n";
my $baseline = $json->decode(do { local $/; <$baseline_file> });
close $baseline_file or die "$baseline_path: $!\n";
$baseline->{schema} == 1 && ref($baseline->{entries}) eq 'ARRAY'
    or die "unsupported documentation debt baseline\n";
my %remaining;
for my $entry (@{$baseline->{entries}}) {
    my $key = $json->encode([@{$entry}{qw(file message source)}]);
    !exists($remaining{$key}) && $entry->{count} =~ /^[1-9][0-9]*$/
        or die "duplicate or invalid documentation debt entry\n";
    $remaining{$key} = $entry->{count};
}
open my $diagnostics, '<', $diagnostics_path or die "$diagnostics_path: $!\n";
my ($complete, $count) = (0, 0);
while (my $line = <$diagnostics>) {
    my $record = $json->decode($line);
    if ($record->{reason} eq 'build-finished') {
        $record->{success} or die "documentation command failed\n";
        $complete++;
    }
    next unless $record->{reason} eq 'compiler-message';
    my $message = $record->{message};
    next unless $message->{level} eq 'warning' || $message->{level} eq 'error';
    ($message->{code}{code} // '') eq 'missing_docs'
        && $message->{level} eq 'warning'
        or die "unexpected documentation diagnostic: $message->{message}\n";
    my @primary = grep { $_->{is_primary} } @{$message->{spans}};
    @primary == 1 or die "ambiguous documentation diagnostic location\n";
    my $span = $primary[0];
    my $source = join "\n", map { $_->{text} } @{$span->{text}};
    my $key = $json->encode([$span->{file_name}, $message->{message}, $source]);
    exists($remaining{$key}) && $remaining{$key}-- > 0
        or die "new documentation debt in $span->{file_name}: $source\n";
    $count++;
}
close $diagnostics or die "$diagnostics_path: $!\n";
$complete == 1 or die "missing or repeated Cargo completion record\n";
print "Private protocol documentation debt admitted: $count existing diagnostics; no growth.\n";
