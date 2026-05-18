<script>
// block comment between default and named imports (prettier moves before comma)
import x,/* c */{a} from './a';

// block comment between comma and namespace (stays after comma)
import y,/* c */* as ns from './b';

// multiple block comments between default and named imports
import v,/* c1 *//* c2 */{b} from './c';
</script>
