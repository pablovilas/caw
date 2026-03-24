interface PluginFilterProps {
  plugins: string[];
  activePlugins: Set<string>;
  onToggle: (plugin: string) => void;
}

export function PluginFilter({ plugins, activePlugins, onToggle }: PluginFilterProps) {
  if (plugins.length <= 1) return null;

  return (
    <div className="plugin-filter">
      {plugins.map((plugin) => (
        <button
          key={plugin}
          className={`filter-btn ${activePlugins.has(plugin) ? "active" : ""}`}
          onClick={() => onToggle(plugin)}
        >
          {plugin}
        </button>
      ))}
    </div>
  );
}
