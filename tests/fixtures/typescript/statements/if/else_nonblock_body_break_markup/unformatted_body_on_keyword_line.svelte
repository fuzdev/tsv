<Button
	onclick={() => {
		if (a) b();
		else c({
			x: 1
		});
	}}
/>

<pre>{(() => {
		if (a) b();
		else c({
			x: 1
		});
	})()}</pre>
