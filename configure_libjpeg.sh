rm -rf target/debug/build/sixel-sys-*
export LDFLAGS=`pkg-config libjpeg --libs`
export CPPFLAGS=`pkg-config libjpeg --cflags`