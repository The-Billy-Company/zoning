Library callers now create one `survey::Session` per invocation and provide it
through `Ask::session`. `Survey::of` returns `Result<Survey>` so unreadable or
unsupported source cannot produce a clean partial graph. Implementations of
`Dialect` now resolve located references through the batch `references` method.
These source API changes require a major version update; the seven contract
laws and CLI policy syntax are unchanged.
