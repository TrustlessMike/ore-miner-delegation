use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use steel::transfer_signed_with_bump;

use crate::{
    consts::LEGACY_BOOST_PROGRAM_ID,
    instruction::UndelegateBoostArgs,
    loaders::{load_delegated_boost_v2, load_managed_proof},
    state::ManagedProof,
    utils::AccountDeserializeV1,
};

pub fn process_legacy_undelegate_boost_v2(
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let [staker, miner, managed_proof_account_info, managed_proof_account_token_account_info, delegate_boost_account_info, boost_account_info, token_mint_account_info, staker_token_account_info, boost_token_account_info, stake_account_info, legacy_boost_program, token_program] =
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

    if *legacy_boost_program.key != LEGACY_BOOST_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    if *token_program.key != spl_token::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let (expected_boost, _) = Pubkey::find_program_address(
        &[b"boost", token_mint_account_info.key.as_ref()],
        &LEGACY_BOOST_PROGRAM_ID,
    );
    if expected_boost != *boost_account_info.key {
        return Err(ProgramError::InvalidSeeds);
    }

    let (expected_stake, _) = Pubkey::find_program_address(
        &[
            b"stake",
            managed_proof_account_info.key.as_ref(),
            boost_account_info.key.as_ref(),
        ],
        &LEGACY_BOOST_PROGRAM_ID,
    );
    if expected_stake != *stake_account_info.key {
        return Err(ProgramError::InvalidSeeds);
    }

    let managed_proof = {
        let data = managed_proof_account_info.data.borrow();
        ManagedProof::try_from_bytes(&data)?.clone()
    };

    if let Ok(mut data) = delegate_boost_account_info.data.try_borrow_mut() {
        let delegated_boost = crate::state::DelegatedBoostV2::try_from_bytes_mut(&mut data)?;

        if amount > delegated_boost.amount {
            return Err(ProgramError::InsufficientFunds);
        }

        delegated_boost.amount = delegated_boost
            .amount
            .checked_sub(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
    } else {
        return Err(ProgramError::AccountBorrowFailed);
    }

    solana_program::program::invoke_signed(
        &Instruction {
            program_id: LEGACY_BOOST_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(*managed_proof_account_info.key, true),
                AccountMeta::new(*managed_proof_account_token_account_info.key, false),
                AccountMeta::new(*boost_account_info.key, false),
                AccountMeta::new(*boost_token_account_info.key, false),
                AccountMeta::new_readonly(*token_mint_account_info.key, false),
                AccountMeta::new(*stake_account_info.key, false),
                AccountMeta::new_readonly(*token_program.key, false),
            ],
            data: [[3_u8].to_vec(), amount.to_le_bytes().to_vec()].concat(),
        },
        &[
            managed_proof_account_info.clone(),
            managed_proof_account_token_account_info.clone(),
            boost_account_info.clone(),
            boost_token_account_info.clone(),
            token_mint_account_info.clone(),
            stake_account_info.clone(),
            token_program.clone(),
        ],
        &[&[
            crate::consts::MANAGED_PROOF,
            miner.key.as_ref(),
            &[managed_proof.bump],
        ]],
    )?;

    let seeds: Vec<&[u8]> = vec![crate::consts::MANAGED_PROOF, miner.key.as_ref()];
    let seeds: &[&[u8]] = &seeds;

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
