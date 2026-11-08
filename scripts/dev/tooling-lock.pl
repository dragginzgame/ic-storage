#!/usr/bin/env perl
use strict;
use warnings;
use JSON::PP;
use File::Temp qw(tempfile);
use File::Basename qw(dirname);
my ($mode, $version, $path, $destination) = @ARGV;
defined($path) && defined($version) && $version =~ /\A(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\z/
    or die "Select an exact X.Y.Z version in root Cargo.toml tooling metadata.\n";
my ($major, $minor) = split /\./, $version;
($major > 0 || $minor >= 34) or die "root tooling selection requires toko-tooling 0.34.0 or later\n";
my $json = JSON::PP->new->canonical;
my $source = 'https://crates.io/crates/toko-tooling';
-f $path && !-l $path or die "expected regular file: $path\n";
open my $fh, '<', $path or die "read $path: $!\n";
my $record;
if ($mode eq 'check') {
    local $/;
    $record = $json->decode(<$fh>);
    ref($record) eq 'HASH' && keys(%$record) == 5
        && ($record->{format} // '') eq '1'
        && ($record->{name} // '') eq 'toko-tooling'
        && ($record->{source} // '') eq $source
        && ($record->{version} // '') eq $version
        && ($record->{checksum} // '') =~ /\A[0-9a-f]{64}\z/
        or die "tooling declaration and lock disagree or lock is invalid; run make shared-tooling-update\n";
} elsif ($mode eq 'resolve') {
    while (my $line = <$fh>) {
        my $entry = $json->decode($line);
        next unless ($entry->{vers} // '') eq $version;
        !defined($record) or die "duplicate registry version\n";
        ($entry->{name} // '') eq 'toko-tooling' && exists($entry->{yanked}) && !$entry->{yanked}
            && ($entry->{cksum} // '') =~ /\A[0-9a-f]{64}\z/
            or die "registry release is yanked or invalid\n";
        $record = {format => 1, name => 'toko-tooling', source => $source,
            version => $version, checksum => $entry->{cksum}};
    }
    defined($record) or die "toko-tooling $version is not available in crates.io\n";
    defined($destination) && !-l $destination && (!-e $destination || -f $destination)
        or die "expected regular tooling lock destination\n";
    my ($out, $temporary) = tempfile('.tooling-lock.XXXXXX', DIR => dirname($destination), UNLINK => 1);
    print {$out} $json->pretty->encode($record) or die "write lock: $!\n";
    close $out or die "close lock: $!\n";
    chmod 0644, $temporary or die "lock mode: $!\n";
    rename $temporary, $destination or die "publish lock: $!\n";
} else { die "usage: tooling-lock.pl check|resolve VERSION INPUT [LOCK]\n"; }
close $fh or die "close $path: $!\n";
print $json->pretty(0)->encode($record), "\n";
