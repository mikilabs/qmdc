import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { LanguageClient, State } from 'vscode-languageclient/node';

/**
 * Tree item types for QMDC Explorer
 */
type QmdcTreeItemType = 'workspace' | 'namespace' | 'object' | 'kind-group' | 'file-group' | 'smart-object';

interface QmdcTreeItem extends vscode.TreeItem {
  itemType: QmdcTreeItemType;
  workspacePath?: string;
  workspaceId?: string;
  namespaceId?: string;
  objectId?: string;
  objectData?: any; // object data from LSP
}

/**
 * TreeDataProvider for QMDC Workspace Explorer.
 * All business logic is in Rust LSP - this is just a thin UI wrapper.
 */
export class QmdcExplorerProvider implements vscode.TreeDataProvider<QmdcTreeItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<QmdcTreeItem | undefined | null | void>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
  
  private lspClient: LanguageClient | undefined;
  private treeData: any = null; // cached tree from LSP
  private groupingMode: 'namespace' | 'file' | 'smart' = 'namespace';
  private outputChannel: vscode.OutputChannel;

  constructor(outputChannel: vscode.OutputChannel) {
    this.outputChannel = outputChannel;
  }

  private log(msg: string): void {
    const timestamp = new Date().toISOString();
    this.outputChannel.appendLine(`[${timestamp}] ${msg}`);
  }

  /**
   * Resolve file path: if absolute, use as-is; if relative, join with workspacePath.
   * Also tries to find the file in workspace folders if it doesn't exist.
   */
  private resolveFilePath(filePath: string, projectRoot: string): string {
    if (!filePath) {
      this.log(`[QMDC Tree] resolveFilePath: empty filePath, projectRoot=${projectRoot}`);
      return projectRoot;
    }

    // If path is already absolute, use it
    if (path.isAbsolute(filePath)) {
      this.log(`[QMDC Tree] resolveFilePath: absolute path=${filePath}`);
      if (fs.existsSync(filePath)) {
        this.log(`[QMDC Tree] resolveFilePath: absolute path exists, returning ${filePath}`);
        return filePath;
      }
      this.log(`[QMDC Tree] resolveFilePath: absolute path not found, returning as-is ${filePath}`);
      return filePath;
    }
    
    // Relative path - LSP returns paths relative to project root (VSCode workspace folder)
    // Simply join with projectRoot
    const fullPath = path.join(projectRoot, filePath);
    this.log(`[QMDC Tree] resolveFilePath: relative path=${filePath}, projectRoot=${projectRoot}, fullPath=${fullPath}`);
    
    if (fs.existsSync(fullPath)) {
      this.log(`[QMDC Tree] resolveFilePath: file exists, returning ${fullPath}`);
      return fullPath;
    }
    
    // Fallback: return the joined path anyway
    this.log(`[QMDC Tree] resolveFilePath: file not found, returning fallback ${fullPath}`);
    return fullPath;
  }

  private computeGlobalId(workspaceId: string, namespaceId: string | undefined, objectId: string): string {
    if (!namespaceId) {
      return `${workspaceId}::${objectId}`;
    }
    return `${workspaceId}:${namespaceId}:${objectId}`;
  }

  setClient(client: LanguageClient | undefined) {
    this.lspClient = client;
    this.refresh();
  }

  refresh(): void {
    this.treeData = null;
    this._onDidChangeTreeData.fire();
  }

  setGroupingMode(mode: 'namespace' | 'file' | 'smart'): void {
    this.groupingMode = mode;
    this.refresh();
  }

  getTreeItem(element: QmdcTreeItem): vscode.TreeItem {
    return element;
  }

  async getChildren(element?: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    if (!this.lspClient || this.lspClient.state !== State.Running) {
      this.log('[QMDC Tree] getChildren: LSP client not running');
      return [];
    }

    // Root level: show workspaces
    if (!element) {
      this.log('[QMDC Tree] getChildren: root level, fetching workspaces');
      return this.getWorkspaces();
    }

    this.log(`[QMDC Tree] getChildren: element type=${element.itemType}, mode=${this.groupingMode}, hasObjectData=${!!element.objectData}`);

    // Workspace level: depends on grouping mode
    if (element.itemType === 'workspace') {
      this.log(`[QMDC Tree] getChildren: workspace element, mode=${this.groupingMode}, wsData=${!!element.objectData}, kindGroups=${element.objectData?.kindGroups?.length || 0}`);
      if (this.groupingMode === 'file') {
        return this.getFileGroups(element);
      } else if (this.groupingMode === 'smart') {
        return this.getSmartObjects(element);
      } else {
        this.log(`[QMDC Tree] getChildren: calling getNamespaces for workspace`);
        return this.getNamespaces(element);
      }
    }

    // Namespace level: show kind groups
    if (element.itemType === 'namespace') {
      return this.getNamespaceKindGroups(element);
    }

    // Kind group level: show objects of this kind
    if (element.itemType === 'kind-group') {
      return this.getKindGroupObjects(element);
    }

    // File group level: show objects in this file
    if (element.itemType === 'file-group') {
      return this.getFileGroupObjects(element);
    }

    // Object level: show children
    if (element.itemType === 'object') {
      return this.getChildObjects(element);
    }

    // Smart object level: show children
    if (element.itemType === 'smart-object') {
      return this.getSmartObjectChildren(element);
    }

    return [];
  }

  /**
   * Fetch tree data from LSP (cached)
   */
  private async fetchTreeData(): Promise<any> {
    if (this.treeData) {
      return this.treeData;
    }

    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders || workspaceFolders.length === 0) {
      this.log('[QMDC Tree] No workspace folders');
      return { success: false };
    }

    const workspacePath = workspaceFolders[0].uri.fsPath;

    try {
      this.log(`[QMDC Tree] Fetching tree data, mode: ${this.groupingMode}`);
      const result = await this.lspClient!.sendRequest('workspace/executeCommand', {
        command: 'qmdc.getWorkspaceTree',
        arguments: ['.', this.groupingMode]
      }) as any;

      this.log(`[QMDC Tree] Result: success=${result?.success}, workspaces=${result?.workspaces?.length || 0}`);
      if (result?.success && result.workspaces?.[0]) {
        const ws = result.workspaces[0];
        this.log(`[QMDC Tree] Workspace ${ws.id}: namespaces=${ws.namespaces?.length || 0}, kindGroups=${ws.kindGroups?.length || 0}, fileGroups=${ws.fileGroups?.length || 0}`);
        // Log full workspace structure for debugging
        this.log(`[QMDC Tree] Full workspace keys: ${Object.keys(ws).join(', ')}`);
        if (ws.namespaces) {
          this.log(`[QMDC Tree] Namespaces: ${JSON.stringify(ws.namespaces.slice(0, 1))}`);
        }
        if (ws.kindGroups) {
          this.log(`[QMDC Tree] KindGroups: ${JSON.stringify(ws.kindGroups.slice(0, 1))}`);
        }
        if (ws.fileGroups) {
          this.log(`[QMDC Tree] FileGroups: ${JSON.stringify(ws.fileGroups.slice(0, 1))}`);
        }
      }

      if (result?.success) {
        this.treeData = result;
      }

      return result;
    } catch (error) {
      this.log(`[QMDC Tree] Failed to fetch workspace tree: ${error}`);
      return { success: false };
    }
  }

  private async getWorkspaces(): Promise<QmdcTreeItem[]> {
    const data = await this.fetchTreeData();
    
    this.log(`[QMDC Tree] getWorkspaces: success=${data?.success}, workspaces=${data?.workspaces?.length || 0}`);
    
    if (!data?.success || !data.workspaces) {
      this.log('[QMDC Tree] getWorkspaces: no data or no workspaces');
      return [];
    }

    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders) {
      this.log('[QMDC Tree] getWorkspaces: no workspace folders');
      return [];
    }

    const items: QmdcTreeItem[] = [];

    for (const ws of data.workspaces) {
      this.log(`[QMDC Tree] getWorkspaces: processing workspace ${ws.id}, namespaces=${ws.namespaces?.length || 0}, kindGroups=${ws.kindGroups?.length || 0}, fileGroups=${ws.fileGroups?.length || 0}`);
      // Use projectRoot from LSP response (enriched from WorkspaceInfo)
      // Fallback to first workspace folder for backward compatibility
      const workspacePath = ws.projectRoot || workspaceFolders[0].uri.fsPath;
      const wsFilePath = ws.projectRoot 
        ? path.join(ws.projectRoot, ws.file)
        : path.join(workspaceFolders[0].uri.fsPath, ws.file);
      const fileUri = vscode.Uri.file(wsFilePath).toString();
      
      // Determine child count based on mode
      let childCount = 0;
      let description = '';
      
      if (this.groupingMode === 'file' && ws.fileGroups) {
        childCount = ws.fileGroups.length;
        description = childCount > 0 ? `${childCount} files` : '';
      } else if (this.groupingMode === 'smart' && ws.objects) {
        childCount = ws.objects.length;
        description = childCount > 0 ? `${childCount} objects` : '';
      } else {
        // namespace mode: count kindGroups + namespaces
        const kindGroupsCount = ws.kindGroups?.length || 0;
        const namespacesCount = ws.namespaces?.length || 0;
        childCount = kindGroupsCount + namespacesCount;
        if (kindGroupsCount > 0 && namespacesCount > 0) {
          description = `${kindGroupsCount} kinds, ${namespacesCount} namespaces`;
        } else if (kindGroupsCount > 0) {
          description = `${kindGroupsCount} kinds`;
        } else if (namespacesCount > 0) {
          description = `${namespacesCount} namespaces`;
        }
      }
      
      const item: QmdcTreeItem = {
        label: ws.label || ws.id,
        itemType: 'workspace',
        workspacePath,
        workspaceId: ws.id,
        objectData: ws,
        collapsibleState: childCount > 0
          ? vscode.TreeItemCollapsibleState.Expanded 
          : vscode.TreeItemCollapsibleState.None,
        iconPath: new vscode.ThemeIcon('folder'),
        description,
        command: {
          command: 'qmdc.goToObjectFromExplorer',
          title: 'Go to Workspace',
          arguments: [fileUri, ws.line != null ? { line: ws.line - 1, character: 0 } : undefined, workspacePath],
        },
      };
      items.push(item);
    }

    return items;
  }

  private async getNamespaces(wsElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = wsElement.workspacePath!;
    const wsData = wsElement.objectData;

    this.log(`[QMDC Tree] getNamespaces: wsData=${!!wsData}, wsData keys=${wsData ? Object.keys(wsData).join(', ') : 'none'}, namespaces=${wsData?.namespaces?.length || 0}, kindGroups=${wsData?.kindGroups?.length || 0}`);
    
    const items: QmdcTreeItem[] = [];

    // First, add top-level kindGroups (objects without namespace grouped by kind) - they should appear ABOVE namespaces
    if (wsData?.kindGroups && wsData.kindGroups.length > 0) {
      this.log(`[QMDC Tree] getNamespaces: adding ${wsData.kindGroups.length} top-level kindGroups`);
      for (const group of wsData.kindGroups) {
        this.log(`[QMDC Tree] getNamespaces: processing kindGroup ${group.kind}, count=${group.count}, objects=${group.objects?.length || 0}`);
        const item: QmdcTreeItem = {
          label: group.label || group.kind,
          itemType: 'kind-group',
          workspacePath,
          workspaceId: wsElement.workspaceId,
          objectData: group,
          collapsibleState: group.objects?.length > 0
            ? vscode.TreeItemCollapsibleState.Collapsed
            : vscode.TreeItemCollapsibleState.None,
          iconPath: new vscode.ThemeIcon('symbol-class'),
          description: group.count > 0 ? `${group.count}` : '',
        };
        items.push(item);
        this.log(`[QMDC Tree] getNamespaces: created item for ${group.kind}, items.length=${items.length}`);
      }
      this.log(`[QMDC Tree] getNamespaces: total items after kindGroups: ${items.length}`);
    } else {
      this.log(`[QMDC Tree] getNamespaces: no kindGroups found, wsData.kindGroups=${wsData?.kindGroups}`);
    }

    // Then, add namespaces
    if (wsData?.namespaces && wsData.namespaces.length > 0) {
      this.log(`[QMDC Tree] getNamespaces: adding ${wsData.namespaces.length} namespaces`);
      for (const ns of wsData.namespaces) {
        const resolvedPath = this.resolveFilePath(ns.file, workspacePath);
        const fileUri = vscode.Uri.file(resolvedPath).toString();
        
        const item: QmdcTreeItem = {
          label: ns.label || ns.id,
          itemType: 'namespace',
          workspacePath,
          workspaceId: wsElement.workspaceId,
          namespaceId: ns.id,
          objectData: ns,
          collapsibleState: ns.kindGroups?.length > 0
            ? vscode.TreeItemCollapsibleState.Collapsed
            : vscode.TreeItemCollapsibleState.None,
          iconPath: new vscode.ThemeIcon('folder-library'),
          description: ns.kindGroups?.length > 0 ? `${ns.kindGroups.length} kinds` : '',
          command: {
            command: 'qmdc.goToObjectFromExplorer',
            title: 'Go to Namespace',
            arguments: [fileUri, ns.line != null ? { line: ns.line - 1, character: 0 } : undefined, workspacePath],
          },
        };
        items.push(item);
      }
    }

    if (items.length === 0) {
      this.log(`[QMDC Tree] getNamespaces: no kindGroups and no namespaces, wsData=${JSON.stringify(wsData ? {id: wsData.id, hasNamespaces: !!wsData.namespaces, hasKindGroups: !!wsData.kindGroups, kindGroupsLength: wsData.kindGroups?.length, keys: Object.keys(wsData)} : null)}`);
    } else {
      this.log(`[QMDC Tree] getNamespaces: returning ${items.length} items`);
    }

    return items;
  }

  private async getNamespaceKindGroups(nsElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = nsElement.workspacePath!;
    const nsData = nsElement.objectData;

    this.log(`[QMDC Tree] getNamespaceKindGroups: nsData=${!!nsData}, kindGroups=${nsData?.kindGroups?.length || 0}`);
    if (!nsData?.kindGroups) {
      this.log(`[QMDC Tree] getNamespaceKindGroups: no kindGroups`);
      return [];
    }

    const items: QmdcTreeItem[] = [];

    for (const group of nsData.kindGroups) {
      const item: QmdcTreeItem = {
        label: group.label || group.kind,
        itemType: 'kind-group',
        workspacePath,
        workspaceId: nsElement.workspaceId,
        namespaceId: nsElement.namespaceId,
        objectData: group,
        collapsibleState: group.objects?.length > 0
          ? vscode.TreeItemCollapsibleState.Collapsed
          : vscode.TreeItemCollapsibleState.None,
        iconPath: new vscode.ThemeIcon('symbol-class'),
        description: group.count > 0 ? `${group.count}` : '',
      };
      items.push(item);
    }

    return items;
  }

  private async getKindGroups(wsElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = wsElement.workspacePath!;
    const wsData = wsElement.objectData;

    this.log(`[QMDC Tree] getKindGroups: wsData=${!!wsData}, kindGroups=${wsData?.kindGroups?.length || 0}`);
    if (!wsData?.kindGroups) {
      return [];
    }

    const items: QmdcTreeItem[] = [];

    for (const group of wsData.kindGroups) {
      const item: QmdcTreeItem = {
        label: group.label || group.kind,
        itemType: 'kind-group',
        workspacePath,
        workspaceId: wsElement.workspaceId,
        objectData: group,
        collapsibleState: group.objects?.length > 0
          ? vscode.TreeItemCollapsibleState.Collapsed
          : vscode.TreeItemCollapsibleState.None,
        iconPath: this.getKindIcon(group.kind),
        description: group.count > 0 ? `${group.count}` : '',
      };
      items.push(item);
    }

    return items;
  }

  private async getFileGroups(wsElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = wsElement.workspacePath!;
    const wsData = wsElement.objectData;

    this.log(`[QMDC Tree] getFileGroups: wsData=${!!wsData}, fileGroups=${wsData?.fileGroups?.length || 0}`);
    if (!wsData?.fileGroups) {
      return [];
    }

    const items: QmdcTreeItem[] = [];

    for (const group of wsData.fileGroups) {
      const resolvedPath = this.resolveFilePath(group.file, workspacePath);
      const fileUri = vscode.Uri.file(resolvedPath).toString();
      
      const item: QmdcTreeItem = {
        label: group.label || group.file,
        itemType: 'file-group',
        workspacePath,
        workspaceId: wsElement.workspaceId,
        objectData: group,
        collapsibleState: group.objects?.length > 0
          ? vscode.TreeItemCollapsibleState.Collapsed
          : vscode.TreeItemCollapsibleState.None,
        iconPath: new vscode.ThemeIcon('file'),
        description: group.count > 0 ? `${group.count}` : '',
        command: {
          command: 'qmdc.goToObjectFromExplorer',
          title: 'Go to File',
          arguments: [fileUri, { line: 0, character: 0 }, workspacePath],
        },
      };
      items.push(item);
    }

    return items;
  }

  private async getObjects(nsElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = nsElement.workspacePath!;
    const workspaceId = nsElement.workspaceId;
    const namespaceId = nsElement.namespaceId;
    const nsData = nsElement.objectData;

    if (!nsData?.objects) {
      return [];
    }

    return this.buildObjectItems(nsData.objects, workspacePath, workspaceId, namespaceId);
  }

  private async getChildObjects(objElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = objElement.workspacePath!;
    const workspaceId = objElement.workspaceId;
    const namespaceId = objElement.namespaceId;
    const objData = objElement.objectData;

    if (!objData?.children) {
      return [];
    }

    return this.buildObjectItems(objData.children, workspacePath, workspaceId, namespaceId);
  }

  private async getKindGroupObjects(groupElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = groupElement.workspacePath!;
    const workspaceId = groupElement.workspaceId;
    const namespaceId = groupElement.namespaceId;
    const groupData = groupElement.objectData;

    if (!groupData?.objects) {
      return [];
    }

    return this.buildObjectItems(groupData.objects, workspacePath, workspaceId, namespaceId);
  }

  private async getFileGroupObjects(groupElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = groupElement.workspacePath!;
    const workspaceId = groupElement.workspaceId;
    const namespaceId = groupElement.namespaceId;
    const groupData = groupElement.objectData;

    if (!groupData?.objects) {
      return [];
    }

    return this.buildObjectItems(groupData.objects, workspacePath, workspaceId, namespaceId);
  }

  private buildObjectItems(objects: any[], workspacePath: string, workspaceId?: string, namespaceId?: string): QmdcTreeItem[] {
    const items: QmdcTreeItem[] = [];

    for (const obj of objects) {
      this.log(`[QMDC Tree] buildObjectItems: id=${obj.id}, kind=${obj.kind}, line=${obj.line}, file=${obj.file}, workspace=${obj.workspace}, namespace=${obj.namespace}`);
      const resolvedPath = this.resolveFilePath(obj.file, workspacePath);
      const fileUri = vscode.Uri.file(resolvedPath).toString();
      
      // Use workspace and namespace from object if available, otherwise fall back to parent context
      const objWorkspaceId = obj.workspace || workspaceId;
      const objNamespaceId = obj.namespace || namespaceId;
      
      const item: QmdcTreeItem = {
        label: obj.label || obj.id,
        itemType: 'object',
        workspacePath,
        workspaceId: objWorkspaceId || undefined,
        namespaceId: objNamespaceId || undefined,
        objectId: obj.id,
        objectData: obj,
        collapsibleState: obj.children?.length > 0
          ? vscode.TreeItemCollapsibleState.Collapsed
          : vscode.TreeItemCollapsibleState.None,
        iconPath: this.getKindIcon(obj.kind),
        description: obj.kind,
        contextValue: 'object',
        command: {
          command: 'qmdc.goToObjectFromExplorer',
          title: 'Go to Object',
          arguments: [fileUri, obj.line != null ? { line: obj.line - 1, character: 0 } : undefined, workspacePath],
        },
      };
      items.push(item);
    }

    return items;
  }

  private async getSmartObjects(workspaceElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = workspaceElement.workspacePath!;
    const wsData = workspaceElement.objectData;

    this.log(`[QMDC Tree] getSmartObjects: wsData=${!!wsData}, objects=${wsData?.objects?.length || 0}`);
    if (!wsData?.objects) {
      return [];
    }

    const items = this.buildSmartObjectItems(wsData.objects, workspacePath);
    this.log(`[QMDC Tree] getSmartObjects: returning ${items.length} items`);
    return items;
  }

  private buildSmartObjectItems(objects: any[], workspacePath: string): QmdcTreeItem[] {
    const items: QmdcTreeItem[] = [];

    this.log(`[QMDC Tree] buildSmartObjectItems: processing ${objects.length} objects`);
    for (const obj of objects) {
      this.log(`[QMDC Tree] buildSmartObjectItems: processing object ${obj.id} (${obj.kind})`);
      const resolvedPath = this.resolveFilePath(obj.file, workspacePath);
      const fileUri = vscode.Uri.file(resolvedPath).toString();

      // Compute __global_id for tooltip
      const workspaceId = obj.workspace || '';
      const namespaceId = obj.namespace;
      const globalId = this.computeGlobalId(workspaceId, namespaceId, obj.id);

      const item: QmdcTreeItem = {
        label: obj.label || obj.id,
        itemType: 'smart-object',
        workspacePath,
        workspaceId: workspaceId || undefined,
        namespaceId: namespaceId || undefined,
        objectId: obj.id,
        objectData: obj,
        collapsibleState: obj.children?.length > 0
          ? vscode.TreeItemCollapsibleState.Collapsed
          : vscode.TreeItemCollapsibleState.None,
        iconPath: this.getKindIcon(obj.kind),
        description: obj.kind,
        tooltip: `🔍 ${globalId}`,
        contextValue: 'smart-object',
        command: {
          command: 'qmdc.goToObjectFromExplorer',
          title: 'Go to Object',
          arguments: [fileUri, obj.line != null ? { line: obj.line - 1, character: 0 } : undefined, workspacePath],
        },
      };
      items.push(item);
    }

    this.log(`[QMDC Tree] buildSmartObjectItems: created ${items.length} items`);
    return items;
  }

  private async getSmartObjectChildren(parentElement: QmdcTreeItem): Promise<QmdcTreeItem[]> {
    const workspacePath = parentElement.workspacePath!;
    const parentData = parentElement.objectData;

    if (!parentData?.children || parentData.children.length === 0) {
      return [];
    }

    return this.buildSmartObjectItems(parentData.children, workspacePath);
  }

  private getKindIcon(kind: string): vscode.ThemeIcon {
    const iconMap: Record<string, string> = {
      // System types
      '__Workspace': 'folder-opened',
      '__Namespace': 'folder-library',
      '__Object': 'symbol-misc',
      '__Document': 'file-text',
      '__TextBlock': 'note',
      
      // Database & Storage
      'Table': 'table',
      'Column': 'symbol-field',
      'Index': 'list-tree',
      'ForeignKey': 'key',
      'Database': 'database',
      'DataSource': 'database',
      'Bucket': 'archive',
      'Schema': 'type-hierarchy',
      
      // Services & Architecture
      'Service': 'server',
      'Endpoint': 'plug',
      'Worker': 'server-process',
      'Component': 'extensions',
      'Integration': 'cloud',
      'API': 'globe',
      
      // Architecture & Design
      'Flow': 'git-compare',
      'Entity': 'symbol-class',
      'Architecture': 'layers',
      'Model': 'symbol-class',
      'Interface': 'symbol-interface',
      
      // UI & User-facing
      'Page': 'browser',
      'Tab': 'layout',
      'Mode': 'symbol-color',
      'Command': 'terminal',
      'View': 'preview',
      
      // Documentation & Queries
      'Query': 'search',
      'Example': 'beaker',
      'Section': 'symbol-namespace',
      'Item': 'circle-outline',
      'Object': 'symbol-misc',
      
      // Users & Teams
      'Actor': 'account',
      'User': 'person',
      'Team': 'organization',
      
      // Goals & Planning
      'Goal': 'target',
      'Scope': 'scope',
      'Task': 'checklist',
      'Milestone': 'milestone',
      
      // Other
      'Config': 'gear',
      'Setting': 'settings-gear',
      'Event': 'bell',
      'Error': 'error',
      'Warning': 'warning',
    };
    return new vscode.ThemeIcon(iconMap[kind] || 'symbol-variable');
  }
}
