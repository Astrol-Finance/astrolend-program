use super::{bank::BankFixture, astrolend_account::AstrolendAccountFixture};
use crate::prelude::{get_oracle_id_from_feed_id, MintFixture};
use crate::utils::*;
use anchor_lang::{prelude::*, solana_program::system_program, InstructionData};

use anyhow::Result;
use astrolend::{
    prelude::AstrolendGroup,
    state::astrolend_group::{BankConfig, BankConfigOpt, BankVaultType, GroupConfig},
};
use solana_program::sysvar;
use solana_program_test::*;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, signature::Keypair,
    signer::Signer, transaction::Transaction,
};
use std::{cell::RefCell, mem, rc::Rc};

pub struct AstrolendGroupFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
}

impl AstrolendGroupFixture {
    pub async fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        config: GroupConfig,
    ) -> AstrolendGroupFixture {
        let ctx_ref = ctx.clone();

        let group_key = Keypair::new();

        {
            let mut ctx = ctx.borrow_mut();

            let initialize_astrolend_group_ix = Instruction {
                program_id: astrolend::id(),
                accounts: astrolend::accounts::AstrolendGroupInitialize {
                    astrolend_group: group_key.pubkey(),
                    admin: ctx.payer.pubkey(),
                    system_program: system_program::id(),
                }
                .to_account_metas(Some(true)),
                data: astrolend::instruction::AstrolendGroupInitialize {}.data(),
            };

            let configure_astrolend_group_ix = Instruction {
                program_id: astrolend::id(),
                accounts: astrolend::accounts::AstrolendGroupConfigure {
                    astrolend_group: group_key.pubkey(),
                    admin: ctx.payer.pubkey(),
                }
                .to_account_metas(Some(true)),
                data: astrolend::instruction::AstrolendGroupConfigure { config }.data(),
            };

            let tx = Transaction::new_signed_with_payer(
                &[initialize_astrolend_group_ix, configure_astrolend_group_ix],
                Some(&ctx.payer.pubkey().clone()),
                &[&ctx.payer, &group_key],
                ctx.last_blockhash,
            );
            ctx.banks_client.process_transaction(tx).await.unwrap();
        }

        AstrolendGroupFixture {
            ctx: ctx_ref.clone(),
            key: group_key.pubkey(),
        }
    }

    pub async fn try_lending_pool_add_bank(
        &self,
        bank_asset_mint_fixture: &MintFixture,
        bank_config: BankConfig,
    ) -> Result<BankFixture, BanksClientError> {
        let bank_key = Keypair::new();
        let bank_mint = bank_asset_mint_fixture.key;
        let bank_fixture =
            BankFixture::new(self.ctx.clone(), bank_key.pubkey(), bank_asset_mint_fixture);

        let mut accounts = astrolend::accounts::LendingPoolAddBank {
            astrolend_group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
            fee_payer: self.ctx.borrow().payer.pubkey(),
            bank_mint,
            bank: bank_key.pubkey(),
            liquidity_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Liquidity).0,
            liquidity_vault: bank_fixture.get_vault(BankVaultType::Liquidity).0,
            insurance_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Insurance).0,
            insurance_vault: bank_fixture.get_vault(BankVaultType::Insurance).0,
            fee_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Fee).0,
            fee_vault: bank_fixture.get_vault(BankVaultType::Fee).0,
            rent: sysvar::rent::id(),
            token_program: bank_asset_mint_fixture.token_program,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true));

        let oracle_key = {
            let oracle_key_or_feed_id = bank_config.oracle_keys[0];
            match bank_config.oracle_setup {
                astrolend::state::price::OracleSetup::PythPushOracle => {
                    get_oracle_id_from_feed_id(oracle_key_or_feed_id).unwrap()
                }
                _ => oracle_key_or_feed_id,
            }
        };

        accounts.push(AccountMeta::new_readonly(oracle_key, false));

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolAddBank {
                bank_config: bank_config.into(),
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
            &[&self.ctx.borrow().payer, &bank_key],
            self.ctx.borrow().last_blockhash,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(bank_fixture)
    }

    pub async fn try_lending_pool_add_bank_with_seed(
        &self,
        bank_asset_mint_fixture: &MintFixture,
        bank_config: BankConfig,
        bank_seed: u64,
    ) -> Result<BankFixture, BanksClientError> {
        let bank_mint = bank_asset_mint_fixture.key;

        // Create PDA account from seeds
        let (pda, _bump) = Pubkey::find_program_address(
            [
                self.key.as_ref(),
                bank_mint.as_ref(),
                &bank_seed.to_le_bytes(),
            ]
            .as_slice(),
            &astrolend::id(),
        );

        let bank_mint = bank_asset_mint_fixture.key;
        let bank_fixture = BankFixture::new(self.ctx.clone(), pda, bank_asset_mint_fixture);

        let mut accounts = astrolend::accounts::LendingPoolAddBankWithSeed {
            astrolend_group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
            fee_payer: self.ctx.borrow().payer.pubkey(),
            bank_mint,
            bank: pda,
            liquidity_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Liquidity).0,
            liquidity_vault: bank_fixture.get_vault(BankVaultType::Liquidity).0,
            insurance_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Insurance).0,
            insurance_vault: bank_fixture.get_vault(BankVaultType::Insurance).0,
            fee_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Fee).0,
            fee_vault: bank_fixture.get_vault(BankVaultType::Fee).0,
            rent: sysvar::rent::id(),
            token_program: bank_fixture.get_token_program(),
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true));

        accounts.push(AccountMeta::new_readonly(bank_config.oracle_keys[0], false));

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolAddBankWithSeed {
                bank_config: bank_config.into(),
                bank_seed,
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
            &[&self.ctx.borrow().payer],
            self.ctx.borrow().last_blockhash,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(bank_fixture)
    }

    pub fn make_lending_pool_configure_bank_ix(
        &self,
        bank: &BankFixture,
        bank_config_opt: BankConfigOpt,
    ) -> Instruction {
        let mut accounts = astrolend::accounts::LendingPoolConfigureBank {
            bank: bank.key,
            astrolend_group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
        }
        .to_account_metas(Some(true));

        if let Some(oracle_config) = bank_config_opt.oracle {
            accounts.extend(
                oracle_config
                    .keys
                    .iter()
                    .map(|k| AccountMeta::new_readonly(*k, false)),
            );
        }

        Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolConfigureBank { bank_config_opt }.data(),
        }
    }

    pub async fn try_lending_pool_configure_bank(
        &self,
        bank: &BankFixture,
        bank_config_opt: BankConfigOpt,
    ) -> Result<(), BanksClientError> {
        let ix = self.make_lending_pool_configure_bank_ix(bank, bank_config_opt);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_accrue_interest(&self, bank: &BankFixture) -> Result<()> {
        let mut ctx = self.ctx.borrow_mut();

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts: astrolend::accounts::LendingPoolAccrueBankInterest {
                astrolend_group: self.key,
                bank: bank.key,
            }
            .to_account_metas(Some(true)),
            data: astrolend::instruction::LendingPoolAccrueBankInterest {}.data(),
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

    pub async fn try_update(&self, config: GroupConfig) -> Result<(), BanksClientError> {
        let ix = Instruction {
            program_id: astrolend::id(),
            accounts: astrolend::accounts::AstrolendGroupConfigure {
                astrolend_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: astrolend::instruction::AstrolendGroupConfigure { config }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_collect_fees(&self, bank: &BankFixture) -> Result<()> {
        let mut ctx = self.ctx.borrow_mut();

        let mut accounts = astrolend::accounts::LendingPoolCollectBankFees {
            astrolend_group: self.key,
            bank: bank.key,
            liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            insurance_vault: bank.get_vault(BankVaultType::Insurance).0,
            fee_vault: bank.get_vault(BankVaultType::Fee).0,
            token_program: bank.get_token_program(),
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == spl_token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolCollectBankFees {}.data(),
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

    pub async fn try_handle_bankruptcy(
        &self,
        bank: &BankFixture,
        astrolend_account: &AstrolendAccountFixture,
    ) -> Result<(), BanksClientError> {
        self.try_handle_bankruptcy_with_nonce(bank, astrolend_account, 100)
            .await
    }

    pub async fn try_handle_bankruptcy_with_nonce(
        &self,
        bank: &BankFixture,
        astrolend_account: &AstrolendAccountFixture,
        nonce: u64,
    ) -> Result<(), BanksClientError> {
        let mut accounts = astrolend::accounts::LendingPoolHandleBankruptcy {
            astrolend_group: self.key,
            signer: self.ctx.borrow().payer.pubkey(),
            bank: bank.key,
            astrolend_account: astrolend_account.key,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            insurance_vault: bank.get_vault(BankVaultType::Insurance).0,
            insurance_vault_authority: bank.get_vault_authority(BankVaultType::Insurance).0,
            token_program: bank.get_token_program(),
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == spl_token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        accounts.append(
            &mut astrolend_account
                .load_observation_account_metas(vec![], vec![])
                .await,
        );

        let mut ctx = self.ctx.borrow_mut();

        let ix = Instruction {
            program_id: astrolend::id(),
            accounts,
            data: astrolend::instruction::LendingPoolHandleBankruptcy {}.data(),
        };

        let nonce_ix = ComputeBudgetInstruction::set_compute_unit_price(nonce);

        let tx = Transaction::new_signed_with_payer(
            &[ix, nonce_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await
    }

    pub fn get_size() -> usize {
        8 + mem::size_of::<AstrolendGroup>()
    }

    pub async fn load(&self) -> astrolend::state::astrolend_group::AstrolendGroup {
        load_and_deserialize::<astrolend::state::astrolend_group::AstrolendGroup>(
            self.ctx.clone(),
            &self.key,
        )
        .await
    }
}
