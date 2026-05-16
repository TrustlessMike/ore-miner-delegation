use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use steel::transfer_signed_with_bump;

use crate::{
    consts::LEGACY_BOOST_PROGRAM,
    instruction::UndelegateBoostArgs,
    loaders::{load_delegated_boost_v2, load_managed_proof},
    state::{DelegatedBoostV2, ManagedProof},
    utils::AccountDeserializeV1,
};

const LEGACY_BOOST_WITHDRAW: u8 = 3;

pub fn process_undelegate_legacy_boost_v2(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let [staker, miner, managed_proof_account_info, managed_proof_account_token_account_info, delegate_boost_account_info, boost_account_info, token_mint_account_info, staker_token_account_info, boost_token_account_info, stake_account_info, ore_boost_program, token_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = UndelegateBoostArgs::try_from_bytes(instruction_data)?;
    let amount = u64::from_le_bytes(args.amount);

    if !staker.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    load_managed_proof(managed_proof_account_info, miner.key, false)?;
    load_delegated_boost_v2(
        delegate_boost_account_info,
        staker.key,
        managed_proof_account_info.key,
        token_mint_account_info.key,
        true,
    )?;

    if *ore_boost_program.key != LEGACY_BOOST_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *token_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let expected_boost = Pubkey::find_program_address(
        &[b"boost", token_mint_account_info.key.as_ref()],
        &LEGACY_BOOST_PROGRAM,
    )
    .0;
    if *boost_account_info.key != expected_boost {
        return Err(ProgramError::InvalidAccountData);
    }

    let expected_boost_tokens = spl_associated_token_account::get_associated_token_address(
        boost_account_info.key,
        token_mint_account_info.key,
    );
    if *boost_token_account_info.key != expected_boost_tokens {
        return Err(ProgramError::InvalidAccountData);
    }

    let expected_stake = Pubkey::find_program_address(
        &[
            b"stake",
            managed_proof_account_info.key.as_ref(),
            boost_account_info.key.as_ref(),
        ],
        &LEGACY_BOOST_PROGRAM,
    )
    .0;
    if *stake_account_info.key != expected_stake {
        return Err(ProgramError::InvalidAccountData);
    }

    let expected_managed_proof_token = spl_associated_token_account::get_associated_token_address(
        managed_proof_account_info.key,
        token_mint_account_info.key,
    );
    if *managed_proof_account_token_account_info.key != expected_managed_proof_token {
        return Err(ProgramError::InvalidAccountData);
    }

    let expected_staker_token = spl_associated_token_account::get_associated_token_address(
        staker.key,
        token_mint_account_info.key,
    );
    if *staker_token_account_info.key != expected_staker_token {
        return Err(ProgramError::InvalidAccountData);
    }

    let managed_proof = {
        let data = managed_proof_account_info.data.borrow();
        *ManagedProof::try_from_bytes(&data)?
    };

    {
        let mut data = delegate_boost_account_info
            .data
            .try_borrow_mut()
            .map_err(|_| ProgramError::AccountBorrowFailed)?;
        let delegated_boost = DelegatedBoostV2::try_from_bytes_mut(&mut data)?;

        if amount > delegated_boost.amount {
            return Err(ProgramError::InsufficientFunds);
        }

        delegated_boost.amount = delegated_boost
            .amount
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
    }

    let mut withdraw_data = vec![LEGACY_BOOST_WITHDRAW];
    withdraw_data.extend_from_slice(&amount.to_le_bytes());

    invoke_signed(
        &Instruction {
            program_id: LEGACY_BOOST_PROGRAM,
            accounts: vec![
                AccountMeta::new(*managed_proof_account_info.key, true),
                AccountMeta::new(*managed_proof_account_token_account_info.key, false),
                AccountMeta::new(*boost_account_info.key, false),
                AccountMeta::new(*boost_token_account_info.key, false),
                AccountMeta::new_readonly(*token_mint_account_info.key, false),
                AccountMeta::new(*stake_account_info.key, false),
                AccountMeta::new_readonly(*token_program.key, false),
            ],
            data: withdraw_data,
        },
        &[
            managed_proof_account_info.clone(),
            managed_proof_account_token_account_info.clone(),
            boost_account_info.clone(),
            boost_token_account_info.clone(),
            token_mint_account_info.clone(),
            stake_account_info.clone(),
            ore_boost_program.clone(),
            token_program.clone(),
        ],
        &[&[
            crate::consts::MANAGED_PROOF,
            miner.key.as_ref(),
            &[managed_proof.bump],
        ]],
    )?;

    let seeds: &[&[u8]] = &[crate::consts::MANAGED_PROOF, miner.key.as_ref()];
    transfer_signed_with_bump(
        managed_proof_account_info,
        managed_proof_account_token_account_info,
        staker_token_account_info,
        token_program,
        amount,
        seeds,
        managed_proof.bump,
    )?;

    Ok(())
}
