// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract PrecompileCaller {
    // Address of the precompile
    address public constant PRECOMPILE_ADDRESS = 0x0000000000_0000000000_0000000000_0000000015; // Replace with the actual precompile address

    /**
     * @dev Calls the precompile with the given input data.
     * @param input The input data to send to the precompile.
     * @return success Whether the call was successful.
     * @return output The output data returned by the precompile.
     */
    function callPrecompile(bytes memory input) public returns (bool success, bytes memory output) {
        (success, output) = PRECOMPILE_ADDRESS.call(input);
    }

    /**
     * @dev Performs a static call to the precompile with the given input data.
     * @param input The input data to send to the precompile.
     * @return success Whether the static call was successful.
     * @return output The output data returned by the precompile.
     */
    function staticCallPrecompile(bytes memory input) public view returns (bool success, bytes memory output) {
        (success, output) = PRECOMPILE_ADDRESS.staticcall(input);
    }
}
