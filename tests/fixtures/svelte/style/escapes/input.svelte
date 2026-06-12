<div>
	CSS escape tests:
	<span class="test-unicode">Unicode</span>
	<span class="test-6digit">6-digit</span>
	<span class="test-with-space">Space</span>
	<span class="test-backslash">Backslash</span>
	<span class="test-adjacent">Adjacent</span>
</div>

<style>
	.test-unicode {
		content: '\41\42\43';
	}

	.test-6digit {
		content: '\01F4A9';
	}

	.test-with-space {
		content: '\41 B';
	}

	.test-backslash {
		content: '\\';
	}

	.test-adjacent {
		content: '\41\42';
	}
</style>
