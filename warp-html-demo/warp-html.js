(function() {
    function loadCSS() {
        const cssPath = 'warp-html.css';
        if (!document.querySelector(`link[href="${cssPath}"]`)) {
            const link = document.createElement('link');
            link.rel = 'stylesheet';
            link.href = cssPath;
            document.head.appendChild(link);
        }
    }

    function tokenize(code) {
        const tokens = [];
        let pos = 0;
        while (pos < code.length) {
            let c = code[pos];
            if (/\s/.test(c)) {
                pos++;
                continue;
            }
            if (c === '"' || c === "'") {
                let quote = c;
                let str = quote;
                pos++;
                while (pos < code.length) {
                    if (code[pos] === '\\') { 
                        str += code[pos];
                        pos++;
                        if (pos < code.length) {
                            str += code[pos];
                            pos++;
                        }
                        continue;
                    }
                    if (code[pos] === quote) {
                        str += code[pos];
                        pos++;
                        break;
                    }
                    str += code[pos];
                    pos++;
                }
                tokens.push(str);
                continue;
            }
            if (['(', ')', ',', ':', '=', '+'].includes(c)) {
                tokens.push(c);
                pos++;
                continue;
            }
            let word = "";
            while (pos < code.length && !/\s/.test(code[pos]) && !['(', ')', ',', ':', '=', '+', '"', "'"].includes(code[pos])) {
                word += code[pos];
                pos++;
            }
            if (word.length > 0) tokens.push(word);
        }
        return tokens;
    }

    function parseCode(code) {
        const tokens = tokenize(code);
        let pos = 0;

        // 【追加】アクション用の関数リスト（これらはUIコンポーネントとして扱わない）
        function isReservedFunc(name) {
            return ['reset', 'calc', 'script', 'add', 'del', 'clr', 'show', 'hide'].includes(name) || name.startsWith('setScreen');
        }

        function parseNode() {
            if (pos >= tokens.length) return null;
            let token = tokens[pos];
            
            if (token.startsWith('@')) {
                let name = token.slice(1);
                let node = { type: 'script', name: name, blocks: [] };
                pos += 2; 
                while(pos < tokens.length && tokens[pos] !== ')') {
                    let blockTypeToken = tokens[pos];
                    if (blockTypeToken === 'if' || blockTypeToken === 'elseIf') {
                        let blockType = blockTypeToken;
                        pos++;
                        if (tokens[pos] === ':') pos++;
                        
                        let condTokens = [];
                        while(pos < tokens.length && tokens[pos] !== '(' && tokens[pos] !== ')') {
                            condTokens.push(tokens[pos]);
                            pos++;
                        }
                        let condition = condTokens.join('');
                        if (tokens[pos] === '(') pos++; 
                        
                        let actionTokens = [];
                        let parenCount = 0;
                        while(pos < tokens.length) {
                            if (parenCount === 0 && tokens[pos] === ')') break;
                            if (tokens[pos] === '(') parenCount++;
                            if (tokens[pos] === ')') parenCount--;
                            actionTokens.push(tokens[pos]);
                            pos++;
                        }
                        node.blocks.push({ type: blockType, condition: condition, actions: actionTokens.join('') });
                        if (tokens[pos] === ')') pos++;
                    } else {
                        pos++;
                    }
                }
                pos++; 
                return node;
            }

            if (pos + 1 < tokens.length && tokens[pos + 1] === '(') {
                let node = {
                    type: 'component',
                    name: token,
                    props: {},
                    events: {},
                    children: []
                };
                pos += 2;
                while (pos < tokens.length && tokens[pos] !== ')') {
                    let t = tokens[pos];
                    
                    // 【修正】子コンポーネントかどうかの判定に isReservedFunc を使用
                    if (pos + 1 < tokens.length && tokens[pos + 1] === '(' && !isReservedFunc(t)) {
                        let child = parseNode();
                        if (child) node.children.push(child);
                        continue;
                    }
                    if (pos + 1 < tokens.length && tokens[pos + 1] === ':') {
                        let key = t;
                        pos += 2;
                        let expr = [];
                        let parenCount = 0;
                        
                        while (pos < tokens.length) {
                            let currentToken = tokens[pos];
                            if (parenCount === 0) {
                                if (currentToken === ')') break;
                                // 【修正】プロパティ読み取りの終了判定にも isReservedFunc を使用
                                if (pos + 1 < tokens.length && tokens[pos + 1] === '(' && !isReservedFunc(currentToken)) break;
                                if (pos + 1 < tokens.length && tokens[pos + 1] === ':') break; 
                            }
                            if (currentToken === '(') parenCount++;
                            else if (currentToken === ')') parenCount--;
                            expr.push(currentToken);
                            pos++;
                        }
                        
                        if (key === 'oneClick' || key === 'longPress') {
                            node.events[key] = expr.join('');
                        } else {
                            node.props[key] = expr;
                        }
                        continue;
                    }
                    pos++;
                }
                pos++;
                return node;
            }
            pos++;
            return null;
        }
        
        const ast = [];
        while (pos < tokens.length) {
            let node = parseNode();
            if (node) ast.push(node);
        }
        return ast;
    }

    function evalExpr(exprArr, state) {
        if (!exprArr || exprArr.length === 0) return "";
        let result = "";
        for (let i = 0; i < exprArr.length; i++) {
            let t = exprArr[i];
            if (t === '+') continue;
            if (t.startsWith('--')) {
                result += state[t] !== undefined ? String(state[t]) : "";
            } else if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'"))) {
                let inner = t.slice(1, -1);
                inner = inner.replace(/\\n/g, '\n').replace(/\\"/g, '"').replace(/\\'/g, "'").replace(/\\\\/g, "\\").replace(/\\ /g, " ").replace(/\\\(/g, "(").replace(/\\\)/g, ")").replace(/\\:/g, ":");
                result += inner;
            } else {
                result += t;
            }
        }
        return result;
    }

    function evaluateRHS(exprStr, state) {
        if (exprStr.startsWith('calc(') && exprStr.endsWith(')')) {
            let inner = exprStr.slice(5, -1);
            inner = inner.replace(/--[a-zA-Z0-9_-]+/g, match => state[match] !== undefined ? String(state[match]) : "");
            try {
                if (!inner.trim()) return "";
                if (/^[0-9+\-*/().\s]*$/.test(inner)) {
                    let res = new Function('return ' + inner)();
                    return (res !== undefined && res !== null) ? String(res) : "";
                }
                return "Error";
            } catch(e) {
                return "Error";
            }
        }

        let parts = [];
        let current = "";
        let inQuote = false;
        for(let i = 0; i < exprStr.length; i++) {
            let c = exprStr[i];
            if (c === '"' || c === "'") {
                inQuote = !inQuote;
                current += c;
            } else if (c === '+' && !inQuote) {
                parts.push(current.trim());
                current = "";
            } else {
                current += c;
            }
        }
        if (current) parts.push(current.trim());

        let result = "";
        for(let p of parts) {
            if ((p.startsWith('"') && p.endsWith('"')) || (p.startsWith("'") && p.endsWith("'"))) {
                let inner = p.slice(1, -1);
                inner = inner.replace(/\\n/g, '\n').replace(/\\"/g, '"').replace(/\\'/g, "'").replace(/\\\\/g, "\\").replace(/\\ /g, " ").replace(/\\\(/g, "(").replace(/\\\)/g, ")").replace(/\\:/g, ":");
                result += inner;
            } else if (p.startsWith('--')) {
                result += state[p] !== undefined ? String(state[p]) : "";
            } else if (p !== "") {
                result += p;
            }
        }
        return result;
    }

    function evaluateCondition(condStr, state) {
        try {
            let parts = condStr.split('=');
            if (parts.length === 2) {
                let left = evalExpr([parts[0].trim()], state);
                let right = evalExpr([parts[1].trim()], state);
                return left === right;
            }
            console.warn(`[Warp Engine] ⚠ 不正な条件式です: '${condStr}'`);
            return false;
        } catch (e) {
            console.error(`[Warp Engine] ❌ 条件の評価に失敗しました '${condStr}':`, e);
            return false;
        }
    }

    function initState(ast, state) {
        if (!state._currentScreen) {
            state._currentScreen = 'main';
        }
        function walk(node) {
            if (node.type === 'script') return;
            for (let key in node.props) {
                if (key.startsWith('--')) {
                    state[key] = evalExpr(node.props[key], state);
                }
            }
            node.children.forEach(walk);
        }
        ast.forEach(walk);
    }

    function initWarp() {
        loadCSS();
        const warpBlocks = document.querySelectorAll('warp-code');
        warpBlocks.forEach((block) => {
            const code = block.textContent;
            const container = document.createElement('div');
            container.className = 'warp-app-container';
            block.parentNode.insertBefore(container, block.nextSibling);
            block.style.display = 'none';
            const state = {};
            const ast = parseCode(code);

            function executeAction(actionStr) {
                let actions = [];
                let currentAct = '';
                let inQuote = false;
                let parenLevel = 0;
                
                for (let i = 0; i < actionStr.length; i++) {
                    let char = actionStr[i];
                    if (char === '"' || char === "'") inQuote = !inQuote;
                    if (!inQuote && char === '(') parenLevel++;
                    if (!inQuote && char === ')') parenLevel--;
                    
                    if (char === ',' && !inQuote && parenLevel === 0) {
                        actions.push(currentAct.trim());
                        currentAct = '';
                    } else {
                        currentAct += char;
                    }
                }
                if (currentAct) actions.push(currentAct.trim());

                for (let act of actions) {
                    let assignIdx = act.indexOf('=');
                    let colonIdx = act.indexOf(':');
                    
                    if (assignIdx === -1 && colonIdx > -1 && act.startsWith('--')) {
                        act = act.slice(0, colonIdx) + '=' + act.slice(colonIdx + 1);
                    }

                    try {
                        if (act.startsWith('add(')) {
                            let inner = act.slice(4, -1);
                            let colIdx = inner.indexOf(':');
                            if (colIdx === -1) throw new Error("add() の形式が間違っています (例: add(targetId: 'component(...)'))");
                            let targetId = inner.slice(0, colIdx).trim();
                            let compStr = inner.slice(colIdx + 1).trim();
                            if (compStr.startsWith("'") || compStr.startsWith('"')) compStr = compStr.slice(1, -1);
                            let dynAst = parseCode(compStr);
                            if (!state._dynamicNodes) state._dynamicNodes = {};
                            if (!state._dynamicNodes[targetId]) state._dynamicNodes[targetId] = [];
                            state._dynamicNodes[targetId].push(...dynAst);
                            console.log(`[Warp Engine] ➕ ノードを追加しました -> ${targetId}`);
                        } else if (act.startsWith('del(')) {
                            let inner = act.slice(4, -1);
                            let colIdx = inner.indexOf(':');
                            let targetId = colIdx > -1 ? inner.slice(0, colIdx).trim() : inner.trim();
                            let compName = colIdx > -1 ? inner.slice(colIdx + 1).trim() : null;
                            if (state._dynamicNodes && state._dynamicNodes[targetId] && state._dynamicNodes[targetId].length > 0) {
                                let list = state._dynamicNodes[targetId];
                                if (compName) {
                                    let removed = false;
                                    for(let i = list.length - 1; i >= 0; i--) {
                                        if (list[i].name === compName) {
                                            list.splice(i, 1);
                                            removed = true;
                                            break;
                                        }
                                    }
                                    if (!removed) console.warn(`[Warp Engine] ⚠ 削除対象 (${compName}) が見つかりません`);
                                } else {
                                    list.pop();
                                }
                                console.log(`[Warp Engine] 🗑 ノードを削除しました <- ${targetId}`);
                            } else {
                                console.warn(`[Warp Engine] ⚠ 削除対象リスト '${targetId}' が存在しないか空です。`);
                            }
                        } else if (act.startsWith('clr(')) {
                            let targetId = act.slice(4, -1).trim();
                            if (state._dynamicNodes) state._dynamicNodes[targetId] = [];
                            console.log(`[Warp Engine] 🧹 リストをクリアしました -> ${targetId}`);
                        } else if (act.startsWith('show(')) {
                            let targetId = act.slice(5, -1).trim();
                            if (!state._visibility) state._visibility = {};
                            state._visibility[targetId] = true;
                            console.log(`[Warp Engine] 👁 表示化 -> ${targetId}`);
                        } else if (act.startsWith('hide(')) {
                            let targetId = act.slice(5, -1).trim();
                            if (!state._visibility) state._visibility = {};
                            state._visibility[targetId] = false;
                            console.log(`[Warp Engine] 🙈 非表示化 -> ${targetId}`);
                        } else if (act.startsWith('script(')) {
                            let scriptName = act.slice(7, -1).trim();
                            executeScript(scriptName);
                        } else if (act.startsWith('reset(')) {
                            setTimeout(() => {
                                let currentScreen = state._currentScreen;
                                Object.keys(state).forEach(k => delete state[k]);
                                state._currentScreen = currentScreen;
                                initState(ast, state);
                                render();
                            }, 500);
                        } else if (act.startsWith('setScreen(')) {
                            let targetScreen = act.slice(10, -1).trim();
                            state._currentScreen = targetScreen;
                        } else if (act.includes('=')) {
                            let parts = act.split('=');
                            let key = parts[0].trim();
                            let valStr = parts.slice(1).join('=').trim();
                            state[key] = evaluateRHS(valStr, state);
                        } else {
                            console.warn(`[Warp Engine] ⚠ 不明なアクションを無視しました: '${act}'`);
                        }
                    } catch (err) {
                        console.error(`[Warp Engine] ❌ アクションの実行に失敗しました '${act}':`, err);
                    }
                }
                render();
            }

            function executeScript(scriptName) {
                let scriptNode = ast.find(n => n.type === 'script' && n.name === scriptName);
                if (!scriptNode) {
                    console.error(`[Warp Engine] ❌ 存在しないスクリプトを呼び出しました: @${scriptName}`);
                    return;
                }
                
                console.groupCollapsed(`[Warp Engine] 📜 スクリプト実行: @${scriptName}`);
                let matchedIf = false;
                let conditionMet = false;

                for (let block of scriptNode.blocks) {
                    if (block.type === 'if') {
                        matchedIf = evaluateCondition(block.condition, state);
                        if (matchedIf) {
                            console.log(`✅ [if: ${block.condition}] 合致しました。処理を実行します。`);
                            conditionMet = true;
                            executeAction(block.actions);
                        } else {
                            console.log(`❌ [if: ${block.condition}] 不一致`);
                        }
                    } else if (block.type === 'elseIf') {
                        if (!matchedIf) {
                            let cond = evaluateCondition(block.condition, state);
                            if (cond) {
                                console.log(`✅ [elseIf: ${block.condition}] 合致しました。処理を実行します。`);
                                matchedIf = true;
                                conditionMet = true;
                                executeAction(block.actions);
                            } else {
                                console.log(`❌ [elseIf: ${block.condition}] 不一致`);
                            }
                        } else {
                            console.log(`⏩ [elseIf: ${block.condition}] 前の条件が合致したためスキップしました`);
                        }
                    }
                }
                
                if (!conditionMet) {
                    console.warn(`⚠ スクリプト @${scriptName} 内で合致する条件がありませんでした`);
                }
                console.groupEnd();
            }

            function renderNode(node) {
                if (node.type === 'script') return document.createDocumentFragment();

                let el;
                switch (node.name) {
                    case 'Header':
                        el = document.createElement('div');
                        el.className = 'warp-header';
                        let title = document.createElement('h1');
                        title.className = 'warp-header-title';
                        title.innerText = evalExpr(node.props['text'], state);
                        el.appendChild(title);
                        let rightEl = document.createElement('div');
                        rightEl.className = 'warp-header-actions';
                        node.children.forEach(c => rightEl.appendChild(renderNode(c)));
                        el.appendChild(rightEl);
                        break;
                    case 'text':
                        el = document.createElement('div');
                        el.className = 'warp-text';
                        el.style.whiteSpace = 'pre-wrap'; 
                        el.innerText = evalExpr(node.props['text'], state);
                        break;
                    case 'card':
                        el = document.createElement('div');
                        el.className = 'warp-card';
                        if (node.props['text']) {
                            let ct = document.createElement('h3');
                            ct.className = 'warp-card-title';
                            ct.innerText = evalExpr(node.props['text'], state);
                            el.appendChild(ct);
                        }
                        node.children.forEach(c => el.appendChild(renderNode(c)));
                        break;
                    case 'hStack':
                        el = document.createElement('div');
                        el.className = 'warp-hstack';
                        node.children.forEach(c => el.appendChild(renderNode(c)));
                        break;
                    case 'vStack':
                        el = document.createElement('div');
                        el.className = 'warp-vstack';
                        el.style.display = 'flex';
                        el.style.flexDirection = 'column';
                        node.children.forEach(c => el.appendChild(renderNode(c)));
                        break;
                    case 'button':
                    case 'tonalButton':
                        el = document.createElement('button');
                        el.className = node.name === 'tonalButton' ? 'warp-button tonal' : 'warp-button';
                        el.innerText = evalExpr(node.props['text'], state);
                        
                        if (evalExpr(node.props['width'], state) === 'max') {
                            el.style.width = '100%';
                        }

                        if (node.events['oneClick']) {
                            el.addEventListener('click', () => executeAction(node.events['oneClick']));
                        }
                        if (node.events['longPress']) {
                            let timer;
                            const startPress = () => {
                                timer = setTimeout(() => executeAction(node.events['longPress']), 600);
                            };
                            const endPress = () => clearTimeout(timer);
                            el.addEventListener('mousedown', startPress);
                            el.addEventListener('mouseup', endPress);
                            el.addEventListener('mouseleave', endPress);
                            el.addEventListener('touchstart', startPress);
                            el.addEventListener('touchend', endPress);
                        }
                        break;
                    default:
                        el = document.createElement('div');
                        el.innerText = `[Unknown Component:${node.name}]`;
                }

                if (el && el.nodeType === 1) {
                    let idVal = evalExpr(node.props['id'], state) || (node.props['id'] ? node.props['id'].join('') : '');
                    if (idVal) {
                        el.id = idVal;
                        
                        if (state._visibility && state._visibility[idVal] === false) {
                            el.style.display = 'none';
                        }
                        
                        if (state._dynamicNodes && state._dynamicNodes[idVal]) {
                            state._dynamicNodes[idVal].forEach(dNode => {
                                el.appendChild(renderNode(dNode));
                            });
                        }
                    }

                    let colorVal = evalExpr(node.props['color'], state) || (node.props['color'] ? node.props['color'].join('') : '');
                    if (colorVal) {
                        const cssColors = {
                            'yellow': '#fbc02d',
                            'red': '#b3261e',
                            'blue': '#0a56d0',
                            'gray': '#808080',
                            'black': '#000000'
                        };
                        let hex = cssColors[colorVal] || colorVal;
                        el.style.setProperty('--warp-color-primary', hex);

                        if (node.name === 'tonalButton') {
                            el.style.color = hex;
                            el.style.backgroundColor = hex + '13';
                        } else if (node.name === 'text' || node.name === 'card') {
                            el.style.color = hex;
                        }
                    } else if (node.name === 'tonalButton') {
                        el.style.color = '#0a56d0';
                        el.style.backgroundColor = '#0a56d013';
                    }
                }
                return el;
            }

            function render() {
                container.innerHTML = ''; 
                ast.forEach(node => {
                    if (node.type === 'script') return;
                    
                    if (node.name === 'screen') {
                        let screenId = node.props['id'] ? node.props['id'].join('') : '';
                        let screenBox = document.createElement('div');
                        screenBox.className = 'warp-screen-box';
                        screenBox.id = 'warp-screen-' + screenId;
                        
                        if (screenId === state._currentScreen) {
                            screenBox.style.display = 'block';
                            node.children.forEach(child => {
                                screenBox.appendChild(renderNode(child));
                            });
                        } else {
                            screenBox.style.display = 'none';
                        }
                        container.appendChild(screenBox);
                    } else {
                        container.appendChild(renderNode(node));
                    }
                });
            }

            initState(ast, state);
            render();
        });
    }
    
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initWarp);
    } else {
        initWarp();
    }
})();