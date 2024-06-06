// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;


import "./Token.sol";
import "./ERC20/IERC20.sol";
import "./Owner/Ownable.sol";
import "./UniswapV3/IUniswapV3Factory.sol";
import "./UniswapV3/IUniswapV3Pool.sol";
import "./UniswapV3/INonfungiblePositionManager.sol";
import "./UniswapV3/TransferHelper.sol";
import "./Math/Math.sol";
import "./ReentrancyGuard.sol";


contract FundingPool is Ownable,ReentrancyGuard  {

    address  private    WETH_ADDRESS = 0x50255C3f96531D9BDb023bCDf1C25FD9BcA271E1;
    IERC20   private    TOKEN;

    address  constant   FEE_ACCOUNT = 0x309500EDfe52f1388C410D4072879820B21Da890;
    address  constant   LP_ACCOUNT = 0x309500EDfe52f1388C410D4072879820B21Da890;

    uint256  constant   MINT_RATE_DECIMALS = 1e9;

    uint8    constant   FEE_RATIO = 5;

    // Uniswap V3 Factory 测试网相关合约地址
    address  constant   UNISWAP_V3_FACTORY = 0x4752ba5DBc23f44D87826276BF6Fd6b1C372aD24; 
    address  constant   POSITION_MANAGER = 0x27F971cb582BF9E50F397e4d29a5C7A34f11faA2;
    uint24   constant   UNISWAP_V3_FEE = 3000;

    // 资金池相关参数
    uint32   immutable  donate_start;
    uint32   immutable  donate_end;
    uint256  immutable  donate_min;
    uint256  immutable  donate_max;
    uint8    immutable  weth_project_ratio;
    uint8    immutable  weth_pool_ratio;
    uint256  immutable  init_mint_rate;
    uint8    immutable  token_project_ratio;
    uint8    immutable  token_pool_ratio;
    uint8    immutable  token_donator_ratio;

    uint256  public     donate_count;

    uint32   public     create_pool_end_time;
    // 资金池衍生参数
    address  public     pool_address;
    // weth 相关
    uint256  public     weth_amount; // 捐赠数量
    uint256  public     pool_weth_amount; //  池子weth数量
    uint256  public     project_weth_amount; // 项目weth数量
    mapping(address=>uint256) private user_weth_amount; // 用户存款数量

    // token相关 
    uint256  public     token_amount; // token 数量
    uint256  public     pool_token_amount; // 池子分配代币
    uint256  public     project_token_amount; // 项目方分配代币
    mapping(address=>uint256)  private user_token_amount; // 用户分配代币
    uint256  public     token_id; // 添加流动性返回的nft_id


    // 事件
    event E_DepositWeth(address indexed wallet, uint256 amount);
    event E_WithdrawWeth(address indexed wallet, uint256 amount);
    event E_RefundWeth(address indexed wallet, uint256 amount);
    event E_WithdrawToken(address indexed wallet, uint256 amount);
    event E_CreateUniPool(address indexed wallet, address pool);


    // 是否可捐赠
    modifier isDonatable {
        require(block.timestamp >= donate_start && block.timestamp <= donate_end && weth_amount < donate_max && pool_address == address(0), "Project completed");
        _;
    }

    // 是否可以退款
    // 1 规定时间没有达到软顶
    // 2 15天内没创建池子
    modifier isRefund{
        require((weth_amount < donate_min && block.timestamp >= donate_end) || (pool_address == address(0) && block.timestamp > create_pool_end_time), "The project not failed.");
        _;

    }

    // 可以创建池子
    modifier canCreatePool{

        require(block.timestamp <= create_pool_end_time && pool_address == address(0) && weth_amount >= donate_min, "Time has elapsed or pool has been created.");
        _;

    }

    // uniswap v3 资金池是否创建
    modifier hasCreatePool{

        require(pool_address != address(0), "The pool has not yet been created");
        _;

    }

    modifier isFeeAccount{

        require(msg.sender == FEE_ACCOUNT, "Not a fee management account");
        _;

    }

    // 构造函数中接收必要信息，并初始化Token合约
    constructor(
        address  _pool_owner,
        uint32 _donate_start,
        uint32 _donate_end,
        uint256 _donate_min,
        uint256 _donate_max,
        uint8 _weth_project_ratio,
        uint8 _weth_pool_ratio,
        uint256 _init_mint_rate,
        uint8 _token_project_ratio,
        uint8 _token_pool_ratio,
        uint8 _token_donator_ratio,
        string memory _tokenName,
        string memory _tokenSymbol

    )Ownable(_pool_owner){
        // 初始化池子参数
        donate_start = _donate_start;
        donate_end = _donate_end;
        donate_min = _donate_min;
        donate_max = _donate_max;
        weth_project_ratio = _weth_project_ratio;
        weth_pool_ratio = _weth_pool_ratio;
        init_mint_rate = _init_mint_rate;
        token_project_ratio = _token_project_ratio;
        token_pool_ratio = _token_pool_ratio;
        token_donator_ratio = _token_donator_ratio;
        create_pool_end_time = donate_end + 60 * 60 * 24 * 15; //
              
        // 初始化代币
        TOKEN = new Token(_tokenName,_tokenSymbol,address(this));
        require(address(TOKEN) != address(0), "Failed to create Token");

    }

    // 存入WETH的函数
    function depositWeth(uint256 amount) external isDonatable {

        if (user_weth_amount[msg.sender] == 0){
            donate_count += 1; // 捐赠人数加一

        }
        _depositWeth(amount);
        emit E_DepositWeth(msg.sender, amount);
        
    }

    // 提取误转的eth函数
    function withdrawEth() external isFeeAccount nonReentrant{

        require(address(this).balance > 0 ,"Insufficient balance to withdraw");
        address payable feeAddress = payable(FEE_ACCOUNT);
        (bool success, ) = feeAddress.call{value: address(this).balance}("");
        require(success, "Failed to withdraw Ether");
        
    }

    // 创建资金池
    function createUniPool() external onlyOwner nonReentrant canCreatePool{

        _createUniPool();
        emit E_CreateUniPool(msg.sender,pool_address);
        
    }

    // 项目方提取WETH,
    function withdrawWeth() external onlyOwner nonReentrant hasCreatePool{

        address owner_ = owner();
        _withdrawWeth(owner_);
        emit E_WithdrawWeth(owner_, project_weth_amount);
        emit E_WithdrawToken(owner_, project_token_amount);

    }

    // 用户退款
    function refundWeth() external nonReentrant isRefund{

        // 检查条件并执行提款
        uint256 weth_balance = user_weth_amount[msg.sender];
        _refundWeth(weth_balance);
        // 触发提款事件
        emit E_RefundWeth(msg.sender, weth_balance);
    }
    
    // 提取代币
    function withdrawToken() external nonReentrant hasCreatePool(){
        // 检查条件并执行提款
        uint256 token_balance = user_token_amount[msg.sender];
        _withdrawToken(token_balance);
        emit E_WithdrawToken(msg.sender, token_balance);      
        
    }

    // 获取token地址
    function getTokenAddress() view external returns(address) {
        return address(TOKEN);
        
    }

    // 获取用户捐赠的WETH
    function getUserWeth() view public returns(uint256){
        return user_weth_amount[msg.sender];
        
    }

    // 获取用户应得Token
    function getUserToken() view public returns(uint256){
        return user_token_amount[msg.sender];
        
    }

    function _depositWeth(uint256 amount) private {

        require(amount > 1e9, "The amount cannot be 0");
        require(amount + weth_amount <= donate_max, "Excess of total maximum donations");
        // 从调用者地址转移WETH到合约
        TransferHelper.safeTransferFrom(WETH_ADDRESS,msg.sender, address(this),amount); 
        
        // 总募捐量更新
        weth_amount += amount;

        // 用户捐赠记录更新
        user_weth_amount[msg.sender] += amount;

        // 池子WETH计算
        uint256 pool_amount = amount * weth_pool_ratio / 100;
        pool_weth_amount += pool_amount;

        // 项目WETH计算
        uint256 project_amount = amount * weth_project_ratio / 100;
        project_weth_amount += project_amount;

        // 代币数量计算  
        uint256 all_tokens = amount * init_mint_rate / MINT_RATE_DECIMALS;
        token_amount += all_tokens;

        uint256 user_tokens = all_tokens * token_donator_ratio / 100;
        user_token_amount[msg.sender] += user_tokens;
        
        uint256 pool_tokens = all_tokens * token_pool_ratio / 100;
        pool_token_amount += pool_tokens;
        
        uint256 project_tokens = all_tokens * token_project_ratio / 100;
        project_token_amount += project_tokens;

        // 铸造代币给合约
        bool mint_success = TOKEN.mint(address(this),all_tokens);
        require(mint_success,"Token minting failure");

        // 断言资产是否正确

        assert(IERC20(WETH_ADDRESS).balanceOf(address(this)) >= weth_amount);

    }

    //添加流动性
    function _initializePool(address _token0,address _token1,uint256 _token0_amount, uint256 _token1_amount) private {

        INonfungiblePositionManager positionManager = INonfungiblePositionManager(POSITION_MANAGER);
        TransferHelper.safeApprove(_token0, POSITION_MANAGER,  _token0_amount); 
        TransferHelper.safeApprove(_token1, POSITION_MANAGER,  _token1_amount); 

        INonfungiblePositionManager.MintParams memory params =
        INonfungiblePositionManager.MintParams({
                token0: _token0,
                token1: _token1,
                fee: UNISWAP_V3_FEE,
                tickLower: -887220,
                tickUpper: 887220,
                amount0Desired: _token0_amount,
                amount1Desired: _token1_amount,
                amount0Min: 0,
                amount1Min: 0,
                recipient:LP_ACCOUNT,
                deadline: block.timestamp + 1000
        });
        (uint mintedId, , , ) = positionManager.mint(params);
        token_id = mintedId;
    }

    function _createUniPool() private{

        require(pool_weth_amount > 0 && pool_token_amount > 0,"Insufficient balance to create pool ");
        // 创建流动池
        IUniswapV3Factory uniswapV3Factory = IUniswapV3Factory(UNISWAP_V3_FACTORY);
        pool_address = uniswapV3Factory.createPool(address(TOKEN),WETH_ADDRESS,UNISWAP_V3_FEE);
        require(pool_address != address(0), "Failed to create Uniswap pool");

        uint256 fee = pool_weth_amount * FEE_RATIO / 100;

        IUniswapV3Pool Pool = IUniswapV3Pool(pool_address);
        address token0 = Pool.token0();
        address token1 = Pool.token1();

        uint256 token0_amount = pool_token_amount;
        uint256 token1_amount = pool_weth_amount - fee;

        if (token0 == WETH_ADDRESS){

            (token0_amount, token1_amount) = (token1_amount, token0_amount);

        }

        uint256 ratioX192 = Math.mulDiv(token1_amount, 1 << 192, token0_amount);
        uint160 sqrtPriceX96 = uint160(Math.sqrt(ratioX192));
        Pool.initialize(sqrtPriceX96);

        // 增加流动性
        pool_weth_amount = 0;
        pool_token_amount = 0;

        _initializePool(token0,token1,token0_amount, token1_amount);
        TransferHelper.safeTransfer(WETH_ADDRESS,FEE_ACCOUNT,fee); 

    }

    function _withdrawWeth(address owner_) private{

        // 检查条件并执行提款
        require(project_weth_amount > 0, "Insufficient balance to refund");

        uint256 fee = project_weth_amount * FEE_RATIO  / 100;
        uint256 project_weth_amount_ = project_weth_amount - fee;

        project_weth_amount = 0;
        TransferHelper.safeTransfer(WETH_ADDRESS,owner_,project_weth_amount_); 
        TransferHelper.safeTransfer(WETH_ADDRESS,FEE_ACCOUNT,fee); 

        // 检查条件并执行提款TOKEN
        require(project_token_amount > 0, "Insufficient balance to refund");

        uint256 project_token_amount_ = project_token_amount;
        project_token_amount = 0;
        TransferHelper.safeTransfer(address(TOKEN),owner_,project_token_amount_); 

    }

    function _refundWeth(uint256 weth_balance) private{

        // 检查条件并执行提款
        require(weth_balance > 0, "Insufficient balance to refund");
        // 减去用户捐款记录中的金额
        user_weth_amount[msg.sender] = 0;
        // 将WETH从合约转移到用户地址
        TransferHelper.safeTransfer(WETH_ADDRESS,msg.sender,weth_balance); 


    }

    function _withdrawToken(uint256 token_balance) private{
        // 检查条件并执行提款
        require(token_balance > 0,"Insufficient balance to withdraw");
        user_token_amount[msg.sender] = 0;
        TransferHelper.safeTransfer(address(TOKEN),msg.sender,token_balance); 
   
        
    }
    
}