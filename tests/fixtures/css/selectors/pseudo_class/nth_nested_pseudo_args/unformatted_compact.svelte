<style>
	/* An+B classification is active in nested pseudo-class args, and restores */
	:is(:not(2n)){color:red}

	/* the flag restores between siblings: .a stays a class after the odd Nth */
	:where(:is(odd),.a){color:blue}
</style>
