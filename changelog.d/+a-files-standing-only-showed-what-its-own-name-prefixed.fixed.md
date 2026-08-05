`zone explain FILE` decided which findings belonged to that file by checking
whether the finding's human-readable subject string started with the file's
path. That only holds by construction for a law whose subject already is
`"{file} -> {target}"`; `use`'s subject is `"{zone or facade} -> {module}"`,
which never starts with a file path, so a file with an ungranted import
always reported a clean standing while `zone verify` on the same package
listed it as broken. An unclaimed-zone violation had the identical gap
(`subject` there is `"unclaimed:{file}"`).

`explain` now matches a finding to a file by the finding's actual recorded
path instead of pattern-matching its prose, converted into the same
repo-relative coordinates `Finding.path` is already reported in. The new
`use`-law property test caught this by generating packages where it was
exercised for the first time at scale.
