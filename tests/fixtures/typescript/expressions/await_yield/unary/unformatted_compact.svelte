<script lang="ts">
	async function fn(){
		/* negation */
		if(!(await a)){}

		/* typeof */
		const t=typeof(await b);
		/* void */
		void(await c);
	}

	function* gen(){
		/* negation */
		if(!(yield a)){}

		/* typeof */
		const t=typeof(yield b);
		/* void */
		void(yield c);
	}
</script>
