# !/bin/sh

cd /tmp
git clone git@github.com:twinexyz/twine-solidity-contracts.git
cd twine-solidity-contracts
# forge build with extra steps
# update version of internal sp1 contract then forge build
./script/updateSp1Version.sh
cd -

cat /tmp/twine-solidity-contracts/out/L1MessageQueue.sol/L1MessageQueue.json | jq -r .abi > crates/evm-contracts/res/L1MessageQueue.json
cat /tmp/twine-solidity-contracts/out/TwineChain.sol/TwineChain.json | jq -r .abi > crates/evm-contracts/res/TwineChain.json
cat /tmp/twine-solidity-contracts/out/L2TwineMessenger.sol/L2TwineMessenger.json | jq -r .abi > crates/evm-contracts/res/L2TwineMessenger.json
