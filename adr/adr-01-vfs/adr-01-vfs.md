# ADR 1: Chain and VFS Path Referencing

## Changelog

- 7DEC2024: Intial draft

## Status

Proposed

## Context

ADOs will need to be able to reference resources locally and on other chains. It needs to be human readable, standardized, and flexible enough for future expansion as the protocol grows. The primary usage of this schema will be for aOS kernel consumption. Optionally, other protocols can use the schema including offchain environments such as a web browser or local command line.

## Decision

Adopt the already standardized URI ([Uniform Resource Indicator](https://en.wikipedia.org/wiki/Uniform_Resource_Identifier_)). We have borrowed many standard from the POSIX frameworks that have been in use since the 1960s and are ubiquitious across all systems worldwide.

![image](https://upload.wikimedia.org/wikipedia/commons/thumb/d/d6/URI_syntax_diagram.svg/2136px-URI_syntax_diagram.svg.png)

- scheme = andr://
- userinfo = aOS username
- host = blockchain (Andromeda Chain being the authority, for now)
- port = n/a for now (\*)
- path = vfs handled by host VFS registry
- query = execution or query parameters
- fragment = n/a for now (\*)

(\*) These are reserved for future use and declared for backwards compatibility.

Examples:

- andr://osmosis/home/alice/splitter?count
- andr://andromeda/etc/blocktime
- andr://cosmos/chain/validators/valoper1
- andr://archway/ibc/channels/12?destination

## Consequences

### ADO/Kernel Updates

A retooling of a ADOs for standardizing this schema may be needed. The parsing and execution of these URIs will be done by the kernel, not the ADO itself.

### Resolvers

Resolvers will need to be established to ensure look ups. For readonly, off chain needs, a local resolver can be used to query a trusted caching/indexing system (graphql) or can be read from a local chain client.

As an example an HTML anchor href (\<a href=""\>) links to an andr:// resource. The andr:// handler could be a web resolver such as [https://andr.zone](https://andr.zone) where a quick tranlation happens:

`andr://osmosis/home/alice/splitter?count` -> `https://osmosis.andr.zone/home/alice/splitter?count` would return either a JSON object or HTML landing page.

### Positive

Easily readable format for referencing any asset inside the aOS network.

### Negative

On and off chain libraries will need to be standardized across the stack. Regex parsing will need to be thoroughly tested. We will need strict enforcement of this standard end-to-end.

### Neutral

## References

- https://en.wikipedia.org/wiki/Uniform_Resource_Identifier
- https://www.icann.org/en/system/files/files/octo-034-27apr22-en.pdf
- https://en.wikipedia.org/wiki/Resource_Description_Framework
- https://en.wikipedia.org/wiki/URL
- https://en.wikipedia.org/wiki/Resource_Description_Framework
