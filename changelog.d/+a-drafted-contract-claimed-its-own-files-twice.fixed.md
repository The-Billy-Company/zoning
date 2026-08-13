`zone draft` gave every zone a recursive `dir/**`, so a directory that both holds
files and has children handed the same file to two zones. On a flat tree nobody
noticed; on a real one the first `zone verify` after a draft opened with thousands of
`claimed by 2 zones` findings, which is the worst possible first impression for a
contract whose whole promise is that it is true of the tree it came from.

A drafted zone now claims the files in its own directory and nothing underneath
(`dir/*.py`), so the zones partition the tree the way the draft says they do. The
generator behind the property tests grows nested directories now too - the shape this
bug needed to appear in was the one shape it could not produce.
