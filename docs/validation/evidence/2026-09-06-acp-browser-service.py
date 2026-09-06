import json,sys
from pathlib import Path
record=Path(sys.argv[1])
for line in sys.stdin:
    request=json.loads(line)
    method=request.get('method')
    with record.open('a') as output:
        output.write(json.dumps({'method':method,'methodId':request.get('params',{}).get('methodId')})+'\n')
    if method=='initialize':
        result={'protocolVersion':1,'agentCapabilities':{},'agentInfo':{'name':'Audit ACP café 日本語','version':'1'},'authMethods':[{'id':'audit','name':'Audit login'}]}
    elif method=='authenticate' and request.get('params',{}).get('methodId')=='audit':
        result={}
    else:
        continue
    print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result}),flush=True)
