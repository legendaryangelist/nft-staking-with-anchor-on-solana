import * as anchor from "@project-serum/anchor"
import { Program } from "@project-serum/anchor"
import { AnchorNftStaking } from "../target/types/anchor_nft_staking"
import { PROGRAM_ID as METADATA_PROGRAM_ID } from "@metaplex-foundation/mpl-token-metadata"
import {
  bundlrStorage,
  CreateNftOutput,
  keypairIdentity,
  Metaplex,
} from "@metaplex-foundation/js"
import { expect } from "chai"
import NodeWallet from "@project-serum/anchor/dist/cjs/nodewallet"

describe("anchor-nft-staking", () => {
  const provider = anchor.AnchorProvider.env()
  anchor.setProvider(provider)

  const program = anchor.workspace.AnchorNftStaking as Program<AnchorNftStaking>
  const wallet = anchor.workspace.AnchorNftStaking.provider.wallet as NodeWallet

  let stakeStatePda: anchor.web3.PublicKey
  let nft: CreateNftOutput;

  it("Setup Test NFT", async () => {
    const payer = wallet.payer
    const metaplex = Metaplex.make(program.provider.connection)
      .use(keypairIdentity(payer))
      .use(bundlrStorage())

    nft = await metaplex
      .nfts()
      .create({
        uri: "",
        name: "Test nft",
        sellerFeeBasisPoints: 0,
      })
      .run();

    stakeStatePda = (await anchor.web3.PublicKey.findProgramAddress(
      [payer.publicKey.toBuffer(), nft.tokenAddress.toBuffer()],
      program.programId
    ))[0];

    console.log("nft metadata pubkey: ", nft.metadataAddress.toBase58());
    console.log("nft token address: ", nft.tokenAddress.toBase58());
    console.log("stake state pda: ", stakeStatePda.toBase58());
  })

  it("Stakes", async () => {
    await program.methods
      .stake()
      .accounts({
        nftTokenAccount: nft.tokenAddress,
        nftMint: nft.mintAddress,
        nftEdition: nft.masterEditionAddress,
        metadataProgram: METADATA_PROGRAM_ID,
      })
      .rpc();

    const account = await program.account.userStakeInfo.fetch(stakeStatePda);
    expect(account.stakeState === "Staked");
  })

  it("Unstakes", async () => {
    await program.methods
      .unstake()
      .accounts({
        nftTokenAccount: nft.tokenAddress,
        nftMint: nft.mintAddress,
        nftEdition: nft.masterEditionAddress,
        metadataProgram: METADATA_PROGRAM_ID,
      })
      .rpc();

    const account = await program.account.userStakeInfo.fetch(stakeStatePda);
    expect(account.stakeState === "Unstaked");
  })
})
