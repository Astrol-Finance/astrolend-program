# Astrolend

## Overview

Astrolend is the first native over-collateralized decentralized lending and borrowing protocol on Eclipse, enabling lenders and borrowers to engage in a decentralized money market.

## Installation

> :warning: astrolend only compiles on the x86_64 architecture. This is to
> ensure struct sizes are always backwards compatible between the SVM and local
> development. Ensure the x86_64 arch is enabled before compiling the project.

Use `anchor build -p astrolend -- --features mainnet-beta` for building the astrolend programs targetting the SVM.

## Architecture

Astrolend's protocol is made up of several key components, each playing a critical role in providing users with a reliable and efficient platform for managing their liquidity.

At the heart of the Astrolend protocol is the astrolend group. This group is a core component that enables users to manage risk and pool their resources to access lending markets more efficiently. Each astrolend group has a lending pool with unlimited banks. Within the lending pool, users can borrow and lend assets, which are then used to generate interest and distribute it among the members of the group. The astrolend group is responsible for managing the risk associated with these activities and ensuring that the borrowing and lending activities are within acceptable risk parameters.

Each bank within the lending pool has its own mint account and a custom oracle, currently limited to Pyth but will soon support Switchboard. This allows Astrolend to tap into multiple sources of liquidity and provide users with access to a diverse range of lending markets. Users can contribute liquidity to the lending pool and earn interest on their contributions. Users can also borrow from the pool to manage their own liquidity needs.

Astrolend accounts are used by users to interact with the protocol. Each astrolend account belongs to a single group and can borrow up to 16 assets simultaneously, providing users with greater flexibility in managing their liquidity. Users can deposit assets into their astrolend account and use them to borrow other assets or lend them to the lending pool. The account balance and borrowing capacity are continuously updated based on user activity and the risk associated with their borrowing and lending activities.

To maintain account health, Astrolend uses a deterministic risk engine that monitors user activity and ensures that borrowing and lending activities are within acceptable risk parameters. The risk engine uses a variety of metrics, including asset prices, volatility, and liquidity, to determine the appropriate risk parameters for each user's astrolend account. If a user's account falls below the minimum required health factor, they may be subject to liquidation to protect the integrity of the lending pool and other users' accounts.

Overall, Astrolend's architecture is designed to provide users with a powerful and flexible platform for managing their liquidity. By leveraging astrolend groups, multiple banks, astrolend accounts, and a robust risk management system, the platform is able to offer competitive interest rates and reliable access to a wide range of lending markets.
