<script lang="ts">
	type Foo = {
		[key: string]: /* c */ B | C;
	};

	type Bar = {
		[key: /* c */ B | C]: string;
	};
</script>
