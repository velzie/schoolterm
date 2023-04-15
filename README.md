# SchoolTerm
a terminal ui for accessing the schooltool grading software

## how do i install this?
```
git clone https://github.com/CoolElectronics/schoolterm
cd schoolterm
cargo build --release
sudo mv target/release/schoolterm /usr/bin/schoolterm
```
## how do i use it?
run `schoolterm` after installing

for the "base url" field, you put in the website you would go to for the school interface, except make sure that if there's a redirect, you put in the path after you get redirected. (eg. https://schooltool.yourschool.edu/schooltoolweb)

the rest is self explanatory i think

make an issue if there's something wrong, but i probably won't fix it any time soon

## why did i make this?
my main motivation for is that the web interface is utter shit and logs you out at random

if you didn't already know, i also have a deep love for tuis and rust, and tuis that happen to be written in rust

## how did i make this?
https://coolelectronics.me/blog/schoolterm
