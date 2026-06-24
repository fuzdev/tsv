<script>
const a = 1;
export {a as 'str name'};
export {'str b'} from './b';
export {'str c' as 'str d'} from './c';
export {'str e' as f} from './e';
export {g as 'str h'} from './g';
export * as 'str ns' from './i';
</script>
