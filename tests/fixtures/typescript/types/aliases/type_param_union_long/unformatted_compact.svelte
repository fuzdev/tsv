<script lang="ts">
	// union `extends` constraint breaks after `extends` with leading-pipe members
	type Constraint<AAAAAAAAAAAAAAAAAAAAAAAAA extends Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Cccccccccccccccccccccccccccccccccc> = X;

	// union default breaks after `=` the same way
	type Default<AAAAAAAAAAAAAAAAAAAAAAAAAAAAA = Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Cccccccccccccccccccccccccccccccccc> = X;
</script>
