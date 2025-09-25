// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IZstdPrecompile {
    function compress(bytes memory) external pure returns (bytes memory);
    function decompress(bytes memory) external pure returns (bytes memory);
}

abstract contract ZstdCompressor {
    address internal immutable ZSTD_PRECOMPILE_ADDRESS;

    bytes4 private constant COMPRESS_SELECTOR = IZstdPrecompile.compress.selector;
    bytes4 private constant DECOMPRESS_SELECTOR = IZstdPrecompile.decompress.selector;

    event Compressed(bytes data);
    event Decompressed(bytes data);

    constructor(address _zstd) {
        ZSTD_PRECOMPILE_ADDRESS = _zstd;
    }

    function _compress(bytes memory data) internal returns (bytes memory ) {
        (bool success, bytes memory result) = ZSTD_PRECOMPILE_ADDRESS.call(
            abi.encodePacked(COMPRESS_SELECTOR, data)
        );
        require(success, "Call failed");
        return result;
    }

    function _decompress(bytes memory data) internal returns (bytes memory) {
        (bool success, bytes memory result) = ZSTD_PRECOMPILE_ADDRESS.call(
            abi.encodePacked(DECOMPRESS_SELECTOR, data)
        );
        require(success, "Call failed");
        return result;
    }
}

contract Compressor is ZstdCompressor {
    constructor() ZstdCompressor(address(0x18)) {}

    function compress(bytes memory data) external {
        emit Compressed(_compress(data));
    }

    function decompress(bytes memory data) external {
        emit Decompressed(_decompress(data));
    }
}
