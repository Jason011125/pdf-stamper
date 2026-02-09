import { FileList } from './components/file-list';
import { PreviewPane } from './components/preview-pane';
import { StampControls } from './components/stamp-controls';

function App(): React.JSX.Element {
  return (
    <div className="flex h-screen bg-gray-50 text-gray-900">
      <div className="w-64 border-r border-gray-200 bg-white p-4">
        <FileList />
      </div>
      <PreviewPane />
      <div className="w-72 border-l border-gray-200 bg-white p-4 overflow-y-auto">
        <StampControls />
      </div>
    </div>
  );
}

export default App;
