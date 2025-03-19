// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {PrecompileCaller} from "../src/PrecompileCaller.sol";

contract PrecompileCallerScript is Script {
    PrecompileCaller public precompileCaller;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        precompileCaller = new PrecompileCaller();

        vm.stopBroadcast();
    }
}
