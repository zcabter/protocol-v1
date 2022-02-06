if [ "$1" != "--skip-build" ]
  then
    anchor build
fi

mkdir -p target/deploy 
cp -R ../target/deploy/ target/deploy
anchor test --skip-build || exit 1;
