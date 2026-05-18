<script lang="ts">
	type F = () => /* c */ string;
	type G = () => /* c */ Promise<string>;
	type H = () => /* c */ string | number;
	type C = new () => /* c */ string;
</script>
