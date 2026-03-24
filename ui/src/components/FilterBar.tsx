interface FilterBarProps {
  plugins: string[];
  activePlugins: Set<string>;
  onToggle: (plugin: string) => void;
}

export function FilterBar({ plugins, activePlugins, onToggle }: FilterBarProps) {
  if (plugins.length <= 1) return null;

  return (
    <div className="filter-bar">
      {plugins.map((plugin) => (
        <button
          key={plugin}
          className={`filter-bar__btn ${activePlugins.has(plugin) ? "filter-bar__btn--active" : ""}`}
          onClick={() => onToggle(plugin)}
        >
          {plugin}
        </button>
      ))}
    </div>
  );
}
