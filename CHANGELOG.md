# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Added Validator Staking ADO [(#330)](https://github.com/andromedaprotocol/andromeda-core/pull/330)
- Added `Asset` enum [(#415)](https://github.com/andromedaprotocol/andromeda-core/pull/415)
- Added `ADOBaseVersion` query to all ADOs [(#416)](https://github.com/andromedaprotocol/andromeda-core/pull/416)
- Staking: Added ability to remove/replace reward token [(#418)](https://github.com/andromedaprotocol/andromeda-core/pull/418)
- Added Expiry Enum [(#419)](https://github.com/andromedaprotocol/andromeda-core/pull/419)
- Added Conditional Splitter [(#441)](https://github.com/andromedaprotocol/andromeda-core/pull/441)
- Validator Staking: Added the option to set an amount while unstaking [(#458)](https://github.com/andromedaprotocol/andromeda-core/pull/458)
- Added Curve ADO [(#515)](https://github.com/andromedaprotocol/andromeda-core/pull/515)
- Set Amount Splitter [(#507)](https://github.com/andromedaprotocol/andromeda-core/pull/507)
- Added String Storage ADO [(#512)](https://github.com/andromedaprotocol/andromeda-core/pull/512)
- Boolean Storage ADO [(#513)](https://github.com/andromedaprotocol/andromeda-core/pull/513)
- Added Counter ADO [(#514)](https://github.com/andromedaprotocol/andromeda-core/pull/514)
- Added Date Time ADO [(#519)](https://github.com/andromedaprotocol/andromeda-core/pull/519)
- Added System ADO - current block [(#526)](https://github.com/andromedaprotocol/andromeda-core/pull/526)
- Added Authorized CW721 Addresses to Marketplace [(#542)](https://github.com/andromedaprotocol/andromeda-core/pull/542)
- Added BuyNow option for Auction [(#533)](https://github.com/andromedaprotocol/andromeda-core/pull/533)
- Added IBC Registry ADO [(#566)](https://github.com/andromedaprotocol/andromeda-core/pull/566)
- Added Denom Validation for Rates [(#568)](https://github.com/andromedaprotocol/andromeda-core/pull/568)
- Added Denom Validation in IBC Registry ADO [(#571)](https://github.com/andromedaprotocol/andromeda-core/pull/571)
- Include ADOBase Version in Schema  [(#574)](https://github.com/andromedaprotocol/andromeda-core/pull/574)
- Added Kernel ICS20 Transfer with Optional ExecuteMsg [(#577)](https://github.com/andromedaprotocol/andromeda-core/pull/577)
- Added IBC Denom Wrap/Unwrap [(#579)](https://github.com/andromedaprotocol/andromeda-core/pull/579)

### Changed

- Merkle Root: stage expiration now uses `Milliseconds`[(#417)](https://github.com/andromedaprotocol/andromeda-core/pull/417)
- Module Redesign [(#452)](https://github.com/andromedaprotocol/andromeda-core/pull/452)
- Primitive Improvements [(#476)](https://github.com/andromedaprotocol/andromeda-core/pull/476)
- Crowdfund Restructure [(#478)](https://github.com/andromedaprotocol/andromeda-core/pull/478)
- Conditional Splitter Internal Audit [(#479)](https://github.com/andromedaprotocol/andromeda-core/pull/479)
- Merkle Root: Andromeda Asset is used now as a `asset_info`[(#480)](https://github.com/andromedaprotocol/andromeda-core/pull/480)
- Reference Address List contract for Permission [(#481)](https://github.com/andromedaprotocol/andromeda-core/pull/481)
- Crowdfund Internal Audit [(#485)](https://github.com/andromedaprotocol/andromeda-core/pull/485)
- Auction: Minimum Raise [(#486)](https://github.com/andromedaprotocol/andromeda-core/pull/486)
- Version Bump [(#488)](https://github.com/andromedaprotocol/andromeda-core/pull/488)
- Made Some CampaignConfig Fields Optional [(#541)](https://github.com/andromedaprotocol/andromeda-core/pull/541)
- Permissioning: Allow multiple actors to be set and removed at once [(#548)](https://github.com/andromedaprotocol/andromeda-core/pull/548)
- Make Action Names in CW721 Conform to Standard [(#545)](https://github.com/andromedaprotocol/andromeda-core/pull/545)
- Timelock ADO: Replace MillisecondsExpiration with Expiry [(#550)](https://github.com/andromedaprotocol/andromeda-core/pull/550)
- Removed staking from vesting contract [(#554)](https://github.com/andromedaprotocol/andromeda-core/pull/554)
- Address List: Support for multiple actors while adding and removing permissions [(#556)](https://github.com/andromedaprotocol/andromeda-core/pull/556)
- ADODB now supports pre-release tagging [(#560)](https://github.com/andromedaprotocol/andromeda-core/pull/560)
- Updated Validator Staking: Updated according to shrelock audit [(#565)](https://github.com/andromedaprotocol/andromeda-core/pull/565)
- Conditional Splitter: Change lock_time's type from MillisecondsDuration to Expiry [(#567)](https://github.com/andromedaprotocol/andromeda-core/pull/567)
- Vesting: Changed to use Milliseconds instead of seconds and removed unnecessary is_multi_batch_enabled flag [(#567)](https://github.com/andromedaprotocol/andromeda-core/pull/578)

### Fixed

- Splitter: avoid zero send messages, owner updates lock any time [(#457)](https://github.com/andromedaprotocol/andromeda-core/pull/457)
- Splitter and Conditional Splitter: fix lock time calculation [(#547)](https://github.com/andromedaprotocol/andromeda-core/pull/547)
- AMPPkt verify origin fix [(#552)](https://github.com/andromedaprotocol/andromeda-core/pull/552)
- Fix permissioning limited use consumptions and blacklist bypass scenario [(#553)](https://github.com/andromedaprotocol/andromeda-core/pull/553)
- Fix precision issue with vestings claim batch method [(#555)](https://github.com/andromedaprotocol/andromeda-core/pull/555)
- Fix shunting ado to work with rust version 1.75.0 [(#565)](https://github.com/andromedaprotocol/andromeda-core/pull/565)

### Removed

- Schemas are no longer tracked [(#430)](https://github.com/andromedaprotocol/andromeda-core/pull/430)
