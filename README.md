# Hướng dẫn tham gia Stake Wars III trên NEAR

# Stake Wars 3 là gì?
Stake Wars giúp NEAR phi tập trung hơn, nâng số lượng active validator từ 100 lên 300, 400, giảm rào cản phần cứng làm validator xuống, qua đó làm mạng lưới phi tập trung và an toàn hơn!
Tham gia Stakewars 3, trở thành chunk-only producers, bạn sẽ có cơ hội nhận incentive với hơn 4 triệu token NEAR làm phần thưởng stake cho những validator tham gia hoàn thành thử thách, mỗi validator sẽ được NEAR stake tối đa 50 nghìn NEAR vào trong vòng ít nhất 1 năm!

# Cấu hình node tham gia Stake Wars khá nhẹ:

    Hardware: Chunk-Only Producer Specifications
    CPU: 4-Core CPU with AVX support
    RAM: 8GB DDR4
    Storage: 500GB SSD
    
bạn có thể thuê ở bất kỳ nhà cung cấp nào, ví dụ như Contabo / Vultr / Amazon Service hay Google Cloud …
Đối với Contabo thì gói 6.99$ là vừa đủ, nhưng để tối ưu nhất hãy chọn gói 11.99$, đăng ký tại: https://contabo.com/en/vps/ 
Chi phí sử thuê VPS sẽ vào khoảng 15$ -> 40$ tùy nhà cung cấp, bạn cần có thẻ thanh toán để đăng ký, ngoài ra bạn có thể sử dụng các dịch vụ VPS tại Việt Nam, một số nhà cung cấp như ViettelCloud / FPT …

![1](https://user-images.githubusercontent.com/36226384/183003699-8a6405c5-e782-4ed2-9aed-64703c7181a8.png)

Hệ điều hành yêu cầu là Ubuntu, toàn bộ hướng dẫn này sẽ chạy trên Ubuntu!

![2](https://user-images.githubusercontent.com/36226384/183067247-275dd87f-1c9c-4953-ba34-1a76e1149714.png)

Quá trình đăng ký rất dễ dàng, sau khi đăng ký bạn hãy lưu lại password login vào vps của mình và địa chỉ IP được cung cấp.

Sau khi có thông tin login, bạn cần sử dụng SSH để login vào máy chủ, nếu sử dụng Windows thì có thể dùng Putty, còn Mac hoặc Linux thì ssh đã có sẵn trong Terminal.


# Tạo thành khoản NEAR trên Shardnet
Truy cập link: https://wallet.shardnet.near.org/

Đăng ký một tài khoản shardnet của bạn, mỗi tài khoản nhận được 50 NEAR test để tham gia mạng lưới. 

![3](https://user-images.githubusercontent.com/36226384/183067624-c10a0d95-eef1-4724-8004-baf43623dd80.jpg)

# Chạy node stakewars 3
Login vào VPS, check nếu CPU hỗ trợ AVX qua câu lệnh

    lscpu | grep -P '(?=.*avx )(?=.*sse4.2 )(?=.*cx16 )(?=.*popcnt )' > /dev/null \
         && echo "Supported" \
         || echo "Not supported"


Check CPU support AVX
Hiện Supported tức là VPS hỗ trợ AVX, nếu không thì bạn cần đăng ký bên nhà cung cấp khác!

# Cập nhật máy chủ
    sudo apt update && sudo apt upgrade -y
# Cài đặt các công cụ dành cho nhà phát triển, Node.js và npm
    curl -sL https://deb.nodesource.com/setup_18.x | sudo -E bash -  
    sudo apt install build-essential nodejs
    PATH="$PATH"
 
# Cài đặt NEAR CLI
    sudo npm install -g near-cli
# Tạo môi trường Shardnet
    export NEAR_ENV=shardnet
    echo 'export NEAR_ENV = shardnet' >> ~ / .bashrc
    echo 'export NEAR_ENV=shardnet' >> ~/.bashrc
    source $HOME/.bash_profile
    
# Tiếp theo, cài đặt các công cụ dành cho nhà phát triển
    sudo apt install -y git binutils-dev libcurl4-openssl-dev zlib1g-dev libdw-dev libiberty-dev cmake gcc g++ python docker.io protobuf-compiler libssl-dev pkg-config clang llvm cargo

# Tiếp theo cài đặt pip Python
    sudo apt install python3-pip
# Tiếp theo Đặt cấu hình
    USER_BASE_BIN=$(python3 -m site --user-base)/bin
    export PATH="$USER_BASE_BIN:$PATH"


