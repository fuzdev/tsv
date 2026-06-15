<script>
	export {a} from 'a' with {type: 'json'};
	export {b, c} from 'b' with {type: 'json'};
	export {d as e} from 'd' with {type: 'json'};
	export {} from 'f' with {type: 'json'};
	export {g} from 'g' with {type: 'json', attr: 'value'};
</script>
