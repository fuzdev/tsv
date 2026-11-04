<script lang="ts">
	(a < readonly(d)[e]) > b;
	(fn < readonly(A)['k']) > x;
	(fn < readonly(A)['k']) > (x, y);
	(fn < readonly(A)) > (x, y);
	(fn < readonly`t`[0]) > `u`;
	(new Cls() < readonly(A)[0]) > x;
	fn<readonly (A | B)['k'][]>(x);
</script>

<p>{(fn < readonly(A)['k']) > (x, y)}</p>
