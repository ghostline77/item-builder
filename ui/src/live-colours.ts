import {
  Color,
  LinearFilter,
  Mesh,
  MeshStandardMaterial,
  SRGBColorSpace,
  TextureLoader,
  Vector2,
} from "three";
import type { Object3D, Texture } from "three";
import type { ColourLayer, Recolour } from "./model";

// Native coefficients include texture detail, microblend and coverage by later
// layers. Only the linear colour multipliers change while the model stays loaded.
export function liveColours() {
  let layers: ColourLayer[] = [],
    edits: Recolour[] = [],
    bindings: { data: ColourLayer; colours: Color[] }[] = [],
    invalidate = () => {},
    generation = 0;
  const update = () => {
    const selected = new Map(edits.map((edit) => [edit.region, edit.colour]));
    for (const binding of bindings)
      binding.colours.forEach((colour, index) => {
        const value = selected.get(binding.data.regions[index] || "");
        if (value) colour.set(value);
        else colour.fromArray(binding.data.colours[index]!);
      });
    invalidate();
  };
  return {
    prepare(data: ColourLayer[]) {
      layers = data;
    },
    set(editsNow: Recolour[]) {
      edits = editsNow;
      update();
    },
    adapt(object: Object3D, redraw: () => void) {
      const token = ++generation;
      bindings = [];
      invalidate = redraw;
      const textures: Texture[] = [],
        seen = new Set<MeshStandardMaterial>();
      object.traverse((child) => {
        if (!(child instanceof Mesh)) return;
        let node: Object3D | null = child;
        while (node && !node.userData.assetId) node = node.parent;
        for (const material of Array.isArray(child.material)
          ? child.material
          : [child.material]) {
          if (
            !(material instanceof MeshStandardMaterial) ||
            seen.has(material) ||
            !material.map
          )
            continue;
          const data = layers.find(
            (layer) =>
              layer.node === node?.userData.assetId &&
              layer.material === material.name,
          );
          if (!data) continue;
          seen.add(material);
          const colours = data.colours.map((value) =>
            new Color().fromArray(value),
          );
          bindings.push({ data, colours });
          const url = URL.createObjectURL(
            new Blob([new Uint8Array(data.png)], { type: "image/png" }),
          );
          const texture = new TextureLoader().load(
            url,
            () => {
              URL.revokeObjectURL(url);
              if (token === generation) redraw();
            },
            undefined,
            () => {
              URL.revokeObjectURL(url);
            },
          );
          texture.colorSpace = SRGBColorSpace;
          texture.flipY = false;
          texture.generateMipmaps = false;
          texture.minFilter = LinearFilter;
          textures.push(texture);
          const columns = data.columns,
            rows = Math.ceil(colours.length / columns);
          material.onBeforeCompile = (shader) => {
            shader.uniforms.glColours = { value: colours };
            shader.uniforms.glCoefficients = { value: texture };
            shader.uniforms.glTileSize = { value: new Vector2(columns, rows) };
            const declarations = `uniform vec3 glColours[${colours.length}];\nuniform sampler2D glCoefficients;\nuniform vec2 glTileSize;\n`;
            shader.fragmentShader =
              declarations +
              shader.fragmentShader.replace(
                "#include <map_fragment>",
                `
              #ifdef USE_MAP
                vec2 glUV=clamp(fract(vMapUv),vec2(0.5/512.0),vec2(1.0-0.5/512.0));
                vec3 glColour=vec3(0.0);
                ${colours.map((_, i) => `glColour+=texture2D(glCoefficients,(glUV+vec2(${i % columns}.0,${Math.floor(i / columns)}.0))/glTileSize).rgb*glColours[${i}];`).join("\n")}
                diffuseColor*=vec4(glColour,1.0);
              #endif
            `,
              );
          };
          material.customProgramCacheKey = () =>
            `ghostline-layers-${colours.length}-${columns}`;
          material.needsUpdate = true;
        }
      });
      update();
      return () => {
        generation++;
        bindings = [];
        textures.forEach((texture) => texture.dispose());
      };
    },
  };
}
