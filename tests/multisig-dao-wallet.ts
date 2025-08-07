import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { MultisigDaoWallet } from "../target/types/multisig_dao_wallet";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("multisig-dao-wallet", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.MultisigDaoWallet as Program<MultisigDaoWallet>;
  const provider = anchor.getProvider();

  // Test accounts
  let authority: Keypair;
  let signer1: Keypair;
  let signer2: Keypair;
  let signer3: Keypair;
  let nonSigner: Keypair;
  let walletConfig: PublicKey;
  let walletConfigBump: number;

  before(async () => {
    // Generate test keypairs
    authority = Keypair.generate();
    signer1 = Keypair.generate();
    signer2 = Keypair.generate();
    signer3 = Keypair.generate();
    nonSigner = Keypair.generate();

    // Airdrop SOL to test accounts
    const airdropAmount = 10 * LAMPORTS_PER_SOL;
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(authority.publicKey, airdropAmount)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(signer1.publicKey, airdropAmount)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(signer2.publicKey, airdropAmount)
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(signer3.publicKey, airdropAmount)
    );

    // Derive wallet config PDA
    [walletConfig, walletConfigBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("wallet_config"), authority.publicKey.toBuffer()],
      program.programId
    );
  });

  describe("Wallet Initialization", () => {
    it("Should initialize wallet with valid parameters", async () => {
      const signers = [signer1.publicKey, signer2.publicKey, signer3.publicKey];
      const threshold = 2;
      const proposalTimeout = new BN(3600); // 1 hour
      const spendingLimit = new BN(1000000000); // 1 SOL
      const spendingPeriod = new BN(86400); // 24 hours

      const tx = await program.methods
        .initializeWallet(signers, threshold, proposalTimeout, spendingLimit, spendingPeriod)
        .accounts({
          walletConfig,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      console.log("Wallet initialization transaction:", tx);

      // Verify wallet config
      const walletConfigAccount = await program.account.walletConfig.fetch(walletConfig);
      expect(walletConfigAccount.authority.toString()).to.equal(authority.publicKey.toString());
      expect(walletConfigAccount.signers.length).to.equal(3);
      expect(walletConfigAccount.threshold).to.equal(threshold);
      expect(walletConfigAccount.isActive).to.be.true;
      expect(walletConfigAccount.members.length).to.equal(3);
      expect(walletConfigAccount.proposalCount).to.equal(0);
    });

    it("Should fail with invalid threshold", async () => {
      const signers = [signer1.publicKey, signer2.publicKey];
      const threshold = 3; // Threshold > number of signers

      try {
        await program.methods
          .initializeWallet(signers, threshold, new BN(3600), new BN(1000000000), new BN(86400))
          .accounts({
            walletConfig: PublicKey.findProgramAddressSync(
              [Buffer.from("wallet_config"), nonSigner.publicKey.toBuffer()],
              program.programId
            )[0],
            authority: nonSigner.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([nonSigner])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error.toString()).to.include("InvalidThreshold");
      }
    });

    it("Should fail with zero threshold", async () => {
      const signers = [signer1.publicKey, signer2.publicKey];
      const threshold = 0;

      try {
        await program.methods
          .initializeWallet(signers, threshold, new BN(3600), new BN(1000000000), new BN(86400))
          .accounts({
            walletConfig: PublicKey.findProgramAddressSync(
              [Buffer.from("wallet_config"), nonSigner.publicKey.toBuffer()],
              program.programId
            )[0],
            authority: nonSigner.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([nonSigner])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error.toString()).to.include("InvalidThreshold");
      }
    });
  });

  describe("Proposal Management", () => {
    let proposal: PublicKey;
    let proposalBump: number;

    before(async () => {
      // Initialize wallet first
      const signers = [signer1.publicKey, signer2.publicKey, signer3.publicKey];
      const threshold = 2;
      const proposalTimeout = new BN(3600);
      const spendingLimit = new BN(1000000000);
      const spendingPeriod = new BN(86400);

      try {
        await program.methods
          .initializeWallet(signers, threshold, proposalTimeout, spendingLimit, spendingPeriod)
          .accounts({
            walletConfig,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([authority])
          .rpc();
      } catch (error) {
        // Wallet might already be initialized, ignore error
        console.log("Wallet already initialized");
      }
    });

    beforeEach(async () => {
      // Create a new proposal for each test
      const walletConfigAccount = await program.account.walletConfig.fetch(walletConfig);
      [proposal, proposalBump] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("proposal"),
          walletConfig.toBuffer(),
          signer1.publicKey.toBuffer(),
        ],
        program.programId
      );
    });

    it("Should create a proposal", async () => {
      const description = "Test proposal for multisig wallet";
      const category = { regular: {} };
      const instructions: any[] = [];
      const expiration = new BN(Math.floor(Date.now() / 1000) + 3600); // 1 hour from now

      const tx = await program.methods
        .addProposal(description, category, instructions, expiration)
        .accounts({
          proposal,
          walletConfig,
          proposer: signer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([signer1])
        .rpc();

      console.log("Proposal creation transaction:", tx);

      // Verify proposal
      const proposalAccount = await program.account.proposal.fetch(proposal);
      expect(proposalAccount.wallet.toString()).to.equal(walletConfig.toString());
      expect(proposalAccount.proposer.toString()).to.equal(signer1.publicKey.toString());
      expect(proposalAccount.description).to.equal(description);
      expect(proposalAccount.status).to.deep.equal({ pending: {} });
      expect(proposalAccount.approvals.length).to.equal(0);
    });

    it("Should approve a proposal", async () => {
      // First create a proposal
      const description = "Test proposal for approval";
      const category = { regular: {} };
      const instructions: any[] = [];
      const expiration = new BN(Math.floor(Date.now() / 1000) + 3600);

      await program.methods
        .addProposal(description, category, instructions, expiration)
        .accounts({
          proposal,
          walletConfig,
          proposer: signer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([signer1])
        .rpc();

      // Approve the proposal
      const tx = await program.methods
        .approveProposal()
        .accounts({
          walletConfig,
          proposal,
          approver: signer1.publicKey,
        })
        .signers([signer1])
        .rpc();

      console.log("Proposal approval transaction:", tx);

      // Verify approval
      const proposalAccount = await program.account.proposal.fetch(proposal);
      expect(proposalAccount.approvals.length).to.equal(1);
      expect(proposalAccount.approvals[0].toString()).to.equal(signer1.publicKey.toString());
    });

    it("Should fail to approve with non-signer", async () => {
      // Create a proposal
      const description = "Test proposal for non-signer approval";
      const category = { regular: {} };
      const instructions: any[] = [];
      const expiration = new BN(Math.floor(Date.now() / 1000) + 3600);

      await program.methods
        .addProposal(description, category, instructions, expiration)
        .accounts({
          proposal,
          walletConfig,
          proposer: signer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([signer1])
        .rpc();

      // Try to approve with non-signer
      try {
        await program.methods
          .approveProposal()
          .accounts({
            walletConfig,
            proposal,
            approver: nonSigner.publicKey,
          })
          .signers([nonSigner])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error.toString()).to.include("NotAuthorized");
      }
    });

    it("Should execute an approved proposal", async () => {
      // Create and approve a proposal
      const description = "Test proposal for execution";
      const category = { regular: {} };
      const instructions: any[] = [];
      const expiration = new BN(Math.floor(Date.now() / 1000) + 3600);

      await program.methods
        .addProposal(description, category, instructions, expiration)
        .accounts({
          proposal,
          walletConfig,
          proposer: signer1.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([signer1])
        .rpc();

      // Approve with enough signers to meet threshold
      await program.methods
        .approveProposal()
        .accounts({
          walletConfig,
          proposal,
          approver: signer1.publicKey,
        })
        .signers([signer1])
        .rpc();

      await program.methods
        .approveProposal()
        .accounts({
          walletConfig,
          proposal,
          approver: signer2.publicKey,
        })
        .signers([signer2])
        .rpc();

      // Execute the proposal
      const tx = await program.methods
        .executeProposal()
        .accounts({
          walletConfig,
          proposal,
          executor: signer1.publicKey,
        })
        .signers([signer1])
        .rpc();

      console.log("Proposal execution transaction:", tx);

      // Verify execution
      const proposalAccount = await program.account.proposal.fetch(proposal);
      expect(proposalAccount.status).to.deep.equal({ executed: {} });
      expect(proposalAccount.executedAt).to.not.be.null;
    });
  });

  describe("Wallet Management", () => {
    before(async () => {
      // Initialize wallet first
      const signers = [signer1.publicKey, signer2.publicKey, signer3.publicKey];
      const threshold = 2;
      const proposalTimeout = new BN(3600);
      const spendingLimit = new BN(1000000000);
      const spendingPeriod = new BN(86400);

      try {
        await program.methods
          .initializeWallet(signers, threshold, proposalTimeout, spendingLimit, spendingPeriod)
          .accounts({
            walletConfig,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([authority])
          .rpc();
      } catch (error) {
        console.log("Wallet already initialized");
      }
    });

    it("Should update signers and threshold", async () => {
      const newSigners = [signer1.publicKey, signer2.publicKey, signer3.publicKey, nonSigner.publicKey];
      const newThreshold = 3;

      const tx = await program.methods
        .updateSigners(newSigners, newThreshold)
        .accounts({
          walletConfig,
          approver: authority.publicKey,
        })
        .signers([authority])
        .rpc();

      console.log("Update signers transaction:", tx);

      // Verify update
      const walletConfigAccount = await program.account.walletConfig.fetch(walletConfig);
      expect(walletConfigAccount.signers.length).to.equal(4);
      expect(walletConfigAccount.threshold).to.equal(newThreshold);
    });

    it("Should set spending limits", async () => {
      const newLimit = new BN(2000000000); // 2 SOL
      const newPeriod = new BN(172800); // 48 hours

      const tx = await program.methods
        .setSpendingLimits(newLimit, newPeriod)
        .accounts({
          walletConfig,
          approver: authority.publicKey,
        })
        .signers([authority])
        .rpc();

      console.log("Set spending limits transaction:", tx);

      // Verify spending limits
      const walletConfigAccount = await program.account.walletConfig.fetch(walletConfig);
      expect(walletConfigAccount.spendingLimit).to.equal(newLimit);
      expect(walletConfigAccount.spendingPeriod).to.equal(newPeriod);
    });

    it("Should delegate vote", async () => {
      const delegate = nonSigner.publicKey;

      const tx = await program.methods
        .delegateVote(delegate)
        .accounts({
          walletConfig,
          delegator: signer1.publicKey,
        })
        .signers([signer1])
        .rpc();

      console.log("Delegate vote transaction:", tx);

      // Verify delegation
      const walletConfigAccount = await program.account.walletConfig.fetch(walletConfig);
      const member = walletConfigAccount.members.find(m => m.address.toString() === signer1.publicKey.toString());
      expect(member?.delegate?.toString()).to.equal(delegate.toString());
    });
  });

  describe("Emergency Override", () => {
    before(async () => {
      // Initialize wallet first
      const signers = [signer1.publicKey, signer2.publicKey, signer3.publicKey];
      const threshold = 2;
      const proposalTimeout = new BN(3600);
      const spendingLimit = new BN(1000000000);
      const spendingPeriod = new BN(86400);

      try {
        await program.methods
          .initializeWallet(signers, threshold, proposalTimeout, spendingLimit, spendingPeriod)
          .accounts({
            walletConfig,
            authority: authority.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([authority])
          .rpc();
      } catch (error) {
        console.log("Wallet already initialized");
      }
    });

    it("Should execute emergency override", async () => {
      const instructions: any[] = [];

      const tx = await program.methods
        .emergencyOverride(instructions)
        .accounts({
          walletConfig,
          emergencyAuthority: authority.publicKey,
        })
        .signers([authority])
        .rpc();

      console.log("Emergency override transaction:", tx);
    });

    it("Should fail emergency override with non-authority", async () => {
      const instructions: any[] = [];

      try {
        await program.methods
          .emergencyOverride(instructions)
          .accounts({
            walletConfig,
            emergencyAuthority: nonSigner.publicKey,
          })
          .signers([nonSigner])
          .rpc();
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error.toString()).to.include("NotAuthorized");
      }
    });
  });
});
