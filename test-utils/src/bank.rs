use super::utils::load_and_deserialize;
use crate::prelude::{
    get_emissions_authority_address, get_emissions_token_account_address, MintFixture,
    TokenAccountFixture,
};
use anchor_lang::{
    prelude::{AccountMeta, Pubkey},
    InstructionData, ToAccountMetas,
};

use fixed::types::I80F48;
use astrolend::{
    bank_authority_seed,
    state::{
        astrolend_group::{Bank, BankConfigOpt, BankVaultType},
        price::{OraclePriceFeedAdapter, OraclePriceType, PriceAdapter},
    },
    utils::{find_bank_vault_authority_pda, find_bank_vault_pda},
};
use solana_program::account_info::IntoAccountInfo;
use solana_program::instruction::Instruction;
use solana_program::sysvar::clock::Clock;
use solana_program_test::BanksClientError;
use solana_program_test::ProgramTestContext;
use solana_sdk::{signer::Signer, transaction::Transaction};
use std::{cell::RefCell, fmt::Debug, rc::Rc};

#[derive(Clone)]
pub struct BankFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
    pub mint: MintFixture,
}

impl BankFixture {
    pub fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        key: Pubkey,
        mint_fixture: &MintFixture,
    ) -> Self {
        Self {
            ctx,
            key,
            mint: mint_fixture.clone(),
        }
    }

    pub fn get_token_program(&self) -> Pubkey {
        self.mint.token_program
    }

    pub fn get_vault(&self, vault_type: BankVaultType) -> (Pubkey, u8) {
        find_bank_vault_pda(&self.key, vault_type)
    }

    pub fn get_vault_authority(&self, vault_type: BankVaultType) -> (Pubkey, u8) {
        find_bank_vault_authority_pda(&self.key, vault_type)
    }

    pub async fn get_price(&self) -> f64 {
        let bank = self.load().await;
        let oracle_key = bank.config.oracle_keys[0];
        let mut oracle_account = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(oracle_key)
            .await
            .unwrap()
            .unwrap();
        let ai = (&oracle_key, &mut oracle_account).into_account_info();
        let oracle_adapter =
            OraclePriceFeedAdapter::try_from_bank_config(&bank.config, &[ai], &Clock::default())
                .unwrap();

        oracle_adapter
            .get_price_of_type(OraclePriceType::RealTime, None)
            .unwrap()
            .to_num()
    }

    pub async fn load(&self) -> Bank {
        load_and_deserialize::<Bank>(self.ctx.clone(), &self.key).await
    }

    pub async fn update_config(&self, config: BankConfigOpt) -> anyhow::Result<()> {
        let mut accounts = astrolend::accounts::LendingPoolConfigureBank {
            astrolend_group: self.load().await.group,
            admin: self.ctx.borrow().payer.pubkey(),
            bank: self.key,
        }
        .to_account_metas(Some(true));

        if let Some(oracle_config) = config.oracle {
            accounts.extend(
                oracle_config
                    .keys
                    .iter()
                    .map(|k| AccountMeta::new_readonly(*k, false)),
            );
        }

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolConfigureBank {
                bank_config_opt: config,
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            self.ctx.borrow().last_blockhash,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(())
    }

    pub async fn try_setup_emissions(
        &self,
        flags: u64,
        rate: u64,
        total_emissions: u64,
        emissions_mint: Pubkey,
        funding_account: Pubkey,
        token_program: Pubkey,
    ) -> Result<(), BanksClientError> {
        let ix = Instruction {
            program_id: astrolend::id(),
            accounts: astrolend::accounts::LendingPoolSetupEmissions {
                astrolend_group: self.load().await.group,
                admin: self.ctx.borrow().payer.pubkey(),
                bank: self.key,
                emissions_mint,
                emissions_funding_account: funding_account,
                emissions_auth: get_emissions_authority_address(self.key, emissions_mint).0,
                emissions_token_account: get_emissions_token_account_address(
                    self.key,
                    emissions_mint,
                )
                .0,
                token_program,
                system_program: solana_program::system_program::id(),
            }
            .to_account_metas(Some(true)),
            data: astrolend::instruction::LendingPoolSetupEmissions {
                rate,
                flags,
                total_emissions,
            }
            .data(),
        };

        let tx = {
            let ctx = self.ctx.borrow_mut();

            Transaction::new_signed_with_payer(
                &[ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                ctx.last_blockhash,
            )
        };

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(())
    }

    pub async fn try_update_emissions(
        &self,
        emissions_flags: Option<u64>,
        emissions_rate: Option<u64>,
        additional_emissions: Option<(u64, Pubkey)>,
        token_program: Pubkey,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts: astrolend::accounts::LendingPoolUpdateEmissionsParameters {
                astrolend_group: self.load().await.group,
                admin: self.ctx.borrow().payer.pubkey(),
                bank: self.key,
                emissions_mint: bank.emissions_mint,
                emissions_funding_account: additional_emissions.map(|(_, f)| f).unwrap_or_default(),
                emissions_token_account: get_emissions_token_account_address(
                    self.key,
                    bank.emissions_mint,
                )
                .0,
                token_program,
            }
            .to_account_metas(Some(true)),
            data: astrolend::instruction::LendingPoolUpdateEmissionsParameters {
                emissions_flags,
                emissions_rate,
                additional_emissions: additional_emissions.map(|(a, _)| a),
            }
            .data(),
        };

        let tx = {
            let ctx = self.ctx.borrow_mut();

            Transaction::new_signed_with_payer(
                &[ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                ctx.last_blockhash,
            )
        };

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(())
    }

    pub async fn try_withdraw_fees(
        &self,
        receiving_account: &TokenAccountFixture,
        amount: u64,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;
        let mut ctx = self.ctx.borrow_mut();
        let signer_pk = ctx.payer.pubkey();
        let (fee_vault_authority, _) = Pubkey::find_program_address(
            bank_authority_seed!(BankVaultType::Fee, self.key),
            &astrolend::id(),
        );

        let mut accounts = astrolend::accounts::LendingPoolWithdrawFees {
            astrolend_group: bank.group,
            token_program: receiving_account.token_program,
            bank: self.key,
            admin: signer_pk,
            fee_vault: bank.fee_vault,
            fee_vault_authority,
            dst_token_account: receiving_account.key,
        }
        .to_account_metas(Some(true));
        if self.mint.token_program == spl_token_2022::ID {
            accounts.push(AccountMeta::new_readonly(self.mint.key, false));
        }

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolWithdrawFees { amount }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_withdraw_insurance(
        &self,
        receiving_account: &TokenAccountFixture,
        amount: u64,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;
        let mut ctx = self.ctx.borrow_mut();
        let signer_pk = ctx.payer.pubkey();
        let (insurance_vault_authority, _) = Pubkey::find_program_address(
            bank_authority_seed!(BankVaultType::Insurance, self.key),
            &astrolend::id(),
        );

        let mut accounts = astrolend::accounts::LendingPoolWithdrawInsurance {
            astrolend_group: bank.group,
            token_program: receiving_account.token_program,
            bank: self.key,
            admin: signer_pk,
            insurance_vault: bank.insurance_vault,
            insurance_vault_authority,
            dst_token_account: receiving_account.key,
        }
        .to_account_metas(Some(true));
        if self.mint.token_program == spl_token_2022::ID {
            accounts.push(AccountMeta::new_readonly(self.mint.key, false));
        }

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolWithdrawInsurance { amount }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn get_vault_token_account(&self, vault_type: BankVaultType) -> TokenAccountFixture {
        let (vault, _) = self.get_vault(vault_type);

        TokenAccountFixture::fetch(self.ctx.clone(), vault).await
    }

    pub async fn set_asset_share_value(&self, value: I80F48) {
        let mut bank_ai = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(self.key)
            .await
            .unwrap()
            .unwrap();
        let bank = bytemuck::from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);

        bank.asset_share_value = value.into();

        self.ctx
            .borrow_mut()
            .set_account(&self.key, &bank_ai.into());
    }
}

impl Debug for BankFixture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BankFixture")
            .field("key", &self.key)
            .finish()
    }
}