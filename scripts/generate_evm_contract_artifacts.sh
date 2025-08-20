# !/bin/sh

cd /tmp
git clone git@github.com:twinexyz/twine-solidity-contracts.git
cd twine-solidity-contracts
# forge build with extra steps
# update version of internal sp1 contract then forge build
./script/updateSp1Version.sh
cd -

cat /tmp/twine-solidity-contracts/out/L1MessageHandler.sol/L1MessageHandler.json | jq -r .abi > crates/evm-contracts/artifacts/L1MessageHandler.json
cat /tmp/twine-solidity-contracts/out/TwineChain.sol/TwineChain.json | jq -r .abi > crates/evm-contracts/artifacts/TwineChain.json
cat /tmp/twine-solidity-contracts/out/L2TwineMessenger.sol/L2TwineMessenger.json | jq -r .abi > crates/evm-contracts/artifacts/L2TwineMessenger.json
cat /tmp/twine-solidity-contracts/out/TwineSystemStorage.sol/TwineSystemStorage.json | jq -r .abi > crates/evm-contracts/artifacts/TwineSystemStorage.json
