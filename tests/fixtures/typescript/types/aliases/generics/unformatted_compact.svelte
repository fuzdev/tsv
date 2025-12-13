<script lang="ts">
	type Box<T> = {value:T};

	type Pair<A,B> = [A,B];

	type Constrained<T extends string> = {
		value:T;
	};

	type WithDefault<T=string> = T|null;

	type IsString<T> = T extends string?true:false;
</script>
