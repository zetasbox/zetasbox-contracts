// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./FundingPool.sol";

contract PoolFactory {
    // 一个新的资金池被创建
    event E_PoolCreated(address indexed new_pool, address indexed creator);
    // 用于存储所有创建的资金池
    mapping(address=>address[]) private all_pools;

    // 创建一个新的资金池合约
    function createPool(
        uint32 donate_start,
        uint32 donate_end,
        uint256 donate_min,
        uint256 donate_max,
        uint8 weth_project_ratio,
        uint8 weth_pool_ratio,
        uint256 init_mint_rate,
        uint8 token_project_ratio,
        uint8 token_pool_ratio,
        uint8 token_donator_ratio,
        string memory tokenName,
        string memory tokenSymbol

    ) external {

        require(weth_pool_ratio >0 token_pool_ratio >0 && ,"Pool ratio must be greater than 0");
        require(weth_pool_ratio + weth_project_ratio == 100,"Weth allocation ratio exceeds error");
        require(token_project_ratio + token_pool_ratio + token_donator_ratio == 100,"Token allocation ratio exceeds error");
        FundingPool new_pool = new FundingPool(
            msg.sender,
            donate_start,
            donate_end,
            donate_min,
            donate_max,
            weth_project_ratio,
            weth_pool_ratio,
            init_mint_rate,
            token_project_ratio,
            token_pool_ratio,
            token_donator_ratio,
            tokenName,
            tokenSymbol
            
        );
        all_pools[msg.sender].push(address(new_pool));
        emit E_PoolCreated(address(new_pool), msg.sender);
    }

    function getPools() view external returns (address[] memory){

        return all_pools[msg.sender];

    }
    
}