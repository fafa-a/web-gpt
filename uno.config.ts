import { defineConfig } from "unocss";
// import presetUno from "@unocss/preset-uno";
import { presetAttributify ,presetUno} from 'unocss'

const entryCSS: CliEntryItem = {
	patterns: ["./templates/*.html"],
	outFile: "./assets/main.css",
};

export default defineConfig({
	cli: {
		entry: entryCSS, // CliEntryItem | CliEntryItem[]
	},
	rules: [],
	presets: [presetAttributify(),presetUno()],
});

interface CliEntryItem {
	/**
	 * Glob patterns to match files
	 */
	patterns: string[];
	/**
	 * The output filename for the generated UnoCSS file
	 */
	outFile: string;
}
