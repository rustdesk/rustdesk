var last_key_time = 0;
var keymap = {};    
for (var (k, v) in Event) {    
    k = k + ""    
    if (k[0] == "V" && k[1] == "K") {       
        keymap[v] = k;        
    }    
}

class Grid: Behavior {
  const TABLE_HEADER_CLICK = 0x81;  
  const TABLE_ROW_CLICK = 0x82; 
  const TABLE_ROW_DBL_CLICK = 0x83; 
  function onHeaderClick(headerCell)    
    {                                          
      this.postEvent(TABLE_HEADER_CLICK, headerCell.index, headerCell);    
      return true;                    
    } 

  function onRowClick(row , reason)        
    {                                                                 
      this.postEvent(TABLE_ROW_CLICK, row.index, row);     
      return true;     
    }              

  function onRowDoubleClick(row)                  
    {                                                             
      this.postEvent(TABLE_ROW_DBL_CLICK, row.index, row);                    
      return true;                                    
    }                  

  function getCurrentRow()                                                                            
    {
      return this.$(tbody>tr:current);
    }

  function getCurrentRows() 
  {
    return this.$$(tbody>tr:checked);
  }
  
  function getCurrentColumn()
  {
    return this.$(thead>:current); // return current cell in header row
  }

  function resetCurrent() {
      var rows = this.getCurrentRows();
      for (var i = 0; i < rows.length; ++i) {
        var row = rows[i];
        row.state.current = false;
        row.state.checked = false;
      }
  }
  
  function setCurrentRow(row, reason = #by_code, doubleClick = false)
    {
      if (!row) return;
      // get previously selected row:
      var prev = this.getCurrentRow();
      if (prev)
      {
        if (prev === row && !doubleClick) return; // already here, nothing to do.
        prev.state.current = false; // drop state flag
        prev.state.checked = false; // drop state flag
      }
      row.state.current = true; 
      row.state.checked = true; 
      row.scrollToView();
     
      if (doubleClick)
        this.onRowDoubleClick(row,reason);
      else
        this.onRowClick(row,reason);
    }
  
  function setCurrentColumn(col)
    {
      // get previously selected column:
      var prev = this.getCurrentColumn();
      if (prev)
      {
        if (prev === col) return; // already here, nothing to do.
        prev.state.current = false; // drop state flag
      }
      col.state.current = true; // set state flag
      col.scrollToView();
      this.onHeaderClick(col);
    }

  function sortRows(sortClicked)
  {                   
      var col = this.sortBy;
      if (!col) return;
      var byColumn = col.index;                                  
      var nowDesc = (col.attributes["sort"] || "desc") == "desc";                                 
      if (sortClicked) (this.$(thead [sort]) || col).attributes["sort"] = undefined; // drop any other sort order.
      var getValue = function(x) {
        var value = x.attributes["value"];
        if (value == undefined) return x.text.toLowerCase();
        return value.toFloat();
      }
      var sort = function(r1, r2, asc) {
        if (r1[1].text == "..") {
          return -1;
        }
        if (r2[1].text == "..") {
          return 1;
        }
        if (!asc) 
          return getValue(r1[byColumn]) < getValue(r2[byColumn]) ? -1 : 1;
        else
          return getValue(r1[byColumn]) > getValue(r2[byColumn]) ? -1 : 1;
      }
      if (nowDesc)
      {                                
        if (sortClicked) col.attributes["sort"] = "asc";                                    
        this.body.sort(:r1, r2: sort(r1, r2, sortClicked ? true : false));
      } else {                          
        if (sortClicked) col.attributes["sort"] = "desc";                                     
        this.body.sort(:r1, r2: sort(r1, r2, sortClicked ? false : true));
      }
  }
  
  function attached()
  {
    assert this.tag == "table" : "wrong element type for grid, table expected";
    this.body = this.$(:root>tbody);
    assert this.body : "Grid require <tbody> element";
  }
  
  function onMouse(evt)
    {
      if ((evt.type != Event.MOUSE_DOWN) && (evt.type != Event.MOUSE_DCLICK))
          return false;
      
      if (!evt.mainButton) 
          return false;
         
      // auxiliary function, returns row this target element belongs to
      function targetRow(target) { return target.$p(tbody>tr); }
         
      // auxiliary function, returns row this target element belongs to
      function targetHeaderCell(target) { return target.$p(thead>tr>th); }
          
      if (var row = targetRow(evt.target)) // click on the row
        this.setCurrentRow(row, #by_mouse, evt.type == Event.MOUSE_DCLICK);
      else if (var headerCell = targetHeaderCell(evt.target))  
      {
        this.setCurrentColumn(headerCell); // click on the header cell
        if (evt.type != Event.MOUSE_DCLICK && headerCell.$is(.sortable)) {
          this.sortBy = headerCell;
          this.sortRows(true);
        }
      }
            
      //return true; // as it is always ours then stop event bubbling
    }

  function onFocus(evt)
  {
      return (evt.type == Event.GOT_FOCUS || evt.type == Event.LOST_FOCUS);
  }
  
  function onKey(evt)
    {
      last_key_time = getTime();
      if (evt.type != Event.KEY_DOWN)
          return false; 
      
      switch(evt.keyCode)
      {
        case Event.VK_DOWN:
          {
             var crow = this.getCurrentRow();
             var idx = crow? crow.index + 1 : 0;
             if (idx < this.body.length) this.setCurrentRow(this.body[idx],#by_key);
          }
          return true;
          
        case Event.VK_UP:             
          {
             var crow = this.getCurrentRow();
             var idx = crow? crow.index - 1 : this.length - 1;
             if (idx >= 0) this.setCurrentRow(this.body[idx],#by_key);
          }
          return true;
  
        case Event.VK_PRIOR:
          {
             var y = this.body.scroll(#top) - this.body.scroll(#height);
             var r;
             for(var i = this.body.length - 1; i >= 0; --i)
             {
                 var pr = r; r = this.body[i];
                 if (r.box(#top, #inner, #content) < y) 
                 {
                    // this row is further than scroll pos - height of scroll area 
                    this.setCurrentRow(pr? pr: r,#by_key); // to last fully visible
                    return true;
                 }
             }
             this.setCurrentRow(r,#by_key); // just in case
          }
          return true;
        case Event.VK_NEXT:
          {
             var y = this.body.scroll(#top) + 2 * this.body.scroll(#height);
             var lastScrollable = this.body.length - 1;
             var r;
             for(var i = 0; i <= lastScrollable; ++i)
             {
                 var pr = r; r = this.body[i];
                 if (r.box(#bottom, #inner, #content) > y)
                 {                               
                    // this row is further than scroll pos - height of scroll area
                    this.setCurrentRow(pr? pr: r,#by_key); // to last fully visible
                    return true;
                 }         
             }
             this.setCurrentRow(r,#by_key); // just in case
          }
          return true;                       
                                                
        case Event.VK_HOME:                                                          
          {
            if (this.body.length)
              this.setCurrentRow(this.body.first,#by_key);
          }                           
          return true;
                                             
        case Event.VK_END:                                    
          {                                                            
            if (this.body.length)
              this.setCurrentRow(this.body.last,#by_key);
          }
          return true;      
      }
      var char = handler.get_char(keymap[evt.keyCode] || "", evt.keyCode);
      if (char) {
        var crow = this.getCurrentRow();
        var idx = crow? crow.index + 1 : 0;
        while (idx < this.body.length) {
          var el = this.body[idx];
          var text = el[1].text;
          if (text && text[0].toLowerCase() == char) {
            this.setCurrentRow(el, #by_key);
            return true;
          }
          idx += 1;
        }
      }
      if (isEnterKey(evt)) {
        this.onRowDoubleClick(this.getCurrentRow());
      }
      return false;
    }              
}
