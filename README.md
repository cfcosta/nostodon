# Nostodon

Nostodon is an application that mirrors the Mastodon federated network into
Nostr. It mirrors users and accounts, allowing Nostr users to follow
interesting people that do not want to switch.

## Motivation

We like Nostr. It is a good idea, and the environment is growing really fast.
However, most of the talk inside it currently is about the protocol, by people
that are building the protocol. This is fine, if that's your jam, but more
"normal" people might want to follow other non-technical people.

Mirroring twitter accounts on Nostr is the obvious target here, but API access
is sparse and shaky (company can change policies at any time). The second
biggest, Mastodon, on the other hand is a nice target, because we can hook on
the federation mechanism and not only sync one user, but the whole network.
