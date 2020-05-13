default:
	cargo build
	#sudo target/debug/ping4 google.com -6 -v 
	#sudo target/debug/ping5
	sudo target/debug/pinglogger \
		google.com \
		facebook.com \
		cs.ubc.ca \
		cs.sfu.ca \
		ec2.us-east-1.amazonaws.com \
		ec2.us-west-2.amazonaws.com

