<b><h20>Hướng dẫn tham gia Stake Wars III trên NEAR</h20></b>

<b><h20>Stake Wars 3 là gì?</h20></b>

Stake Wars giúp NEAR phi tập trung hơn, nâng số lượng active validator từ 100 lên 300, 400, giảm rào cản phần cứng làm validator xuống, qua đó làm mạng lưới phi tập trung và an toàn hơn!

Tham gia Stakewars 3, trở thành chunk-only producers, bạn sẽ có cơ hội nhận incentive với hơn 4 triệu token NEAR làm phần thưởng stake cho những validator tham gia hoàn thành thử thách, mỗi validator sẽ được NEAR stake tối đa 50 nghìn NEAR vào trong vòng ít nhất 1 năm!


Cấu hình node tham gia Stake Wars khá nhẹ:

Hardware: Chunk-Only Producer Specifications
CPU: 4-Core CPU with AVX support
RAM: 8GB DDR4
Storage: 500GB SSD


# Yiimp_install_scrypt (update Feb 4, 2018)


Discord : https://discord.gg/zcCXjkQ

TUTO Youtube : https://www.youtube.com/watch?v=vdBCw6_cyig

Official Yiimp (used in this script for Yiimp Installation): https://github.com/tpruvot/yiimp


***********************************

## Install script for yiimp on Ubuntu 16.04

Connect on your VPS =>
- adduser pool
- adduser pool sudo
- su - pool
- git clone https://github.com/xavatar/yiimp_install_scrypt.git
- cd yiimp_install_scrypt/
- sudo bash install.sh (Do not run the script as root)
- sudo bash screen-scrypt.sh (in tuto youtube, i launch the scrypt with root... it does not matter)
- sudo bash screen-stratum.sh (configure before start this script... add or remove algo you use) 

Finish !
Go http://xxx.xxxxxx.xxx and Enjoy !

###### :bangbang: **YOU MUST UPDATE THE FOLLOWING FILES :**
- **/var/web/serverconfig.php :** update this file to include your public ip to access the admin panel. update with public keys from exchanges. update with other information specific to your server..
- **/etc/yiimp/keys.php :** update with secrect keys from the exchanges


###### :bangbang: **IMPORTANT** : 

- Your mysql information (login/Password) is saved in **~/.my.cnf**
- **If you reboot your VPS**, you must restart screen-scrypt.sh and screen-stratum.sh (or add crontab)
- Remember to restart **memcached service** after the db change (update or import new .sql)

***********************************

###### This script has an interactive beginning and will ask for the following information :

- Enter time zone
- Server Name 
- Are you using a subdomain
- Enter support email
- Set stratum to AutoExchange
- New location for /site/adminRights
- Your Public IP for admin access
- Install Fail2ban
- Install UFW and configure ports
- Install LetsEncrypt SSL

***********************************

**This install script will get you 95% ready to go with yiimp. There are a few things you need to do after the main install is finished.**

While I did add some server security to the script, it is every server owners responsibility to fully secure their own servers. After the installation you will still need to customize your serverconfig.php file to your liking, add your API keys, and build/add your coins to the control panel. 

There will be several wallets already in yiimp. These have nothing to do with the installation script and are from the database import from the yiimp github. 

If you need further assistance we have a small but growing discord channel at https://discord.gg/zcCXjkQ

If this helped you or you feel giving please donate BTC Donation: 1PqjApUdjwU9k4v1RDWf6XveARyEXaiGUz
