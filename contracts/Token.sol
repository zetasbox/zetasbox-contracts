// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./ERC20/ERC20.sol";
import "./Owner/Ownable.sol";

contract Token is ERC20,Ownable {
    constructor(string memory name, string memory symbol,address _token_owner) ERC20(name, symbol) Ownable(_token_owner) {
        
    }

    function mint(address to, uint256 amount) public onlyOwner returns(bool){
        _mint(to, amount);
        return true;
    }
}