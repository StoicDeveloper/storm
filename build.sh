cargo +nightly build
flutter_rust_bridge_codegen --rust-input storm_frontend/rust/src/lib.rs \
	                        --dart-output storm_frontend/lib/bridge.dart \
							--rust-output storm_frontend/rust/src/bridge.rs \
							--class-name 'StormController'
