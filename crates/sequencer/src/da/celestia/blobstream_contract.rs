use alloy_sol_types::sol;

// SP1 Blobstream contract ABI
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract SP1Blobstream {
        struct DataRootTuple {
            // Celestia block height the data root was included in.
            uint256 height;
            // Data root.
            bytes32 dataRoot;
        }

        /// Merkle Tree Proof structure.
        struct BinaryMerkleProof {
            // List of side nodes to verify and calculate tree.
            bytes32[] sideNodes;
            // The key of the leaf to verify.
            uint256 key;
            // The number of leaves in the tree
            uint256 numLeaves;
        }

        uint64 public latestBlock;

        function verifyAttestation(uint256 _proofNonce, DataRootTuple memory _tuple, BinaryMerkleProof memory _proof) external view returns (bool);
    }
}
