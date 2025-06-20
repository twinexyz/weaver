// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract Cat {
    event Meow(bytes x);
    bytes internal recorded;

    function speak(bytes memory x) external {
        recorded = x;
        emit Meow(x);
    }

    function getRecording() external view returns(bytes memory) {
        return recorded;
    }

    function returnSelector(bytes memory x) external pure returns(bytes memory) {
        return abi.encodeWithSelector(this.speak.selector, x);
    }
}