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
the official schooltool web interface is rendered server side, and there's no publicly documented api, so i had to use the mobile app's endpoints, with a combination of packet inspection and reverse engineering. httptoolkit for getting the endpoints, and ilspy to pull apart the android apk to make my own implementation of their goofy aah password algorithm

credit to whoever made ./decomp.py, i forgot where i found it but you need it for decompressing the .net assembly.
an extracted and decompresed schooltool.dll is included in the repo if you want to hop in


this repository uses console_engine. i would not advise you do the same
